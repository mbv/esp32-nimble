use super::ble_gap_conn_find;
use crate::{
  ble,
  utilities::{mutex::Mutex, BleUuid},
  BLECharacteristic, BLEDevice, BLEReturnCode, BLEService, NimbleProperties,
};
use alloc::{boxed::Box, sync::Arc, vec::Vec};
use core::ffi::c_void;

const BLE_HS_CONN_HANDLE_NONE: u16 = esp_idf_sys::BLE_HS_CONN_HANDLE_NONE as _;

#[allow(clippy::type_complexity)]
pub struct BLEServer {
  pub(crate) started: bool,
  advertise_on_disconnect: bool,
  services: Vec<Arc<Mutex<BLEService>>>,
  notify_characteristic: Vec<&'static mut BLECharacteristic>,
  connections: Vec<u16>,
  indicate_wait: [u16; esp_idf_sys::CONFIG_BT_NIMBLE_MAX_CONNECTIONS as _],

  on_connect: Option<Box<dyn FnMut(&esp_idf_sys::ble_gap_conn_desc) + Send + Sync>>,
  on_disconnect: Option<Box<dyn FnMut(&esp_idf_sys::ble_gap_conn_desc) + Send + Sync>>,
}

impl BLEServer {
  pub(crate) fn new() -> Self {
    Self {
      started: false,
      advertise_on_disconnect: true,
      services: Vec::new(),
      notify_characteristic: Vec::new(),
      connections: Vec::new(),
      indicate_wait: [BLE_HS_CONN_HANDLE_NONE; esp_idf_sys::CONFIG_BT_NIMBLE_MAX_CONNECTIONS as _],
      on_connect: None,
      on_disconnect: None,
    }
  }

  pub fn start(&mut self) -> Result<(), BLEReturnCode> {
    if self.started {
      return Ok(());
    }

    for svc in &mut self.services {
      svc.lock().start()?;
    }

    unsafe {
      ble!(esp_idf_sys::ble_gatts_start())?;

      for svc in &self.services {
        let mut svc = svc.lock();
        ble!(esp_idf_sys::ble_gatts_find_svc(
          &svc.uuid.u,
          &mut svc.handle
        ))?;

        for chr in &svc.characteristics {
          let mut chr = chr.lock();
          if chr
            .properties
            .intersects(NimbleProperties::Indicate | NimbleProperties::Notify)
          {
            let chr = &mut *chr;
            self
              .notify_characteristic
              .push(super::extend_lifetime_mut(chr));
          }
        }
      }
    }

    self.started = true;

    Ok(())
  }

  pub fn connected_count(&self) -> usize {
    self.connections.len()
  }

  pub fn create_service(&mut self, uuid: BleUuid) -> Arc<Mutex<BLEService>> {
    let service = Arc::new(Mutex::new(BLEService::new(uuid)));
    self.services.push(service.clone());
    service
  }

  pub(crate) extern "C" fn handle_gap_event(
    event: *mut esp_idf_sys::ble_gap_event,
    _arg: *mut c_void,
  ) -> i32 {
    let event = unsafe { &*event };
    let server = BLEDevice::take().get_server();

    match event.type_ as _ {
      esp_idf_sys::BLE_GAP_EVENT_CONNECT => {
        let connect = unsafe { &event.__bindgen_anon_1.connect };
        if connect.status == 0 {
          server.connections.push(connect.conn_handle);

          if let Ok(desc) = ble_gap_conn_find(connect.conn_handle) {
            if let Some(callback) = server.on_connect.as_mut() {
              callback(&desc);
            }
          }
        }
      }
      esp_idf_sys::BLE_GAP_EVENT_DISCONNECT => {
        let disconnect = unsafe { &event.__bindgen_anon_1.disconnect };
        if let Some(idx) = server
          .connections
          .iter()
          .position(|x| *x == disconnect.conn.conn_handle)
        {
          server.connections.swap_remove(idx);
        }

        if let Some(callback) = server.on_disconnect.as_mut() {
          callback(&disconnect.conn);
        }

        if server.advertise_on_disconnect {
          if let Err(err) = BLEDevice::take().get_advertising().start() {
            ::log::warn!("can't start advertising: {:?}", err);
          }
        }
      }
      esp_idf_sys::BLE_GAP_EVENT_SUBSCRIBE => {
        let subscribe = unsafe { &event.__bindgen_anon_1.subscribe };
        if let Some(chr) = server
          .notify_characteristic
          .iter_mut()
          .find(|x| x.handle == subscribe.attr_handle)
        {
          chr.subscribe(subscribe);
        }
      }
      esp_idf_sys::BLE_GAP_EVENT_NOTIFY_TX => {
        let notify_tx = unsafe { &event.__bindgen_anon_1.notify_tx };
        #[allow(unused_variables)]
        if let Some(chr) = server
          .notify_characteristic
          .iter()
          .find(|x| x.handle == notify_tx.attr_handle)
        {
          #[allow(clippy::collapsible_if)]
          if notify_tx.indication() > 0 {
            if notify_tx.status != 0 {
              server.clear_indicate_wait(notify_tx.conn_handle);
            }
          }
        }
      }
      _ => {}
    }

    0
  }

  pub(super) fn set_indicate_wait(&self, conn_handle: u16) -> bool {
    !self.indicate_wait.contains(&conn_handle)
  }

  pub(super) fn clear_indicate_wait(&mut self, conn_handle: u16) {
    if let Some(it) = self.indicate_wait.iter_mut().find(|x| **x == conn_handle) {
      *it = BLE_HS_CONN_HANDLE_NONE;
    }
  }
}