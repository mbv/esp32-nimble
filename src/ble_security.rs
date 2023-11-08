use crate::enums;

pub struct BLESecurity {
  passkey: u32,
}

impl BLESecurity {
  pub(crate) fn new() -> Self {
    Self { passkey: 0 }
  }

  pub fn set_auth(&mut self, bonding: bool, mitm: bool, sc: bool) -> &mut Self {
    unsafe {
      esp_idf_sys::ble_hs_cfg.set_sm_bonding(bonding as _);
      esp_idf_sys::ble_hs_cfg.set_sm_mitm(mitm as _);
      esp_idf_sys::ble_hs_cfg.set_sm_sc(sc as _);
    }

    self
  }

  pub fn get_passkey(&self) -> u32 {
    self.passkey
  }

  pub fn set_passkey(&mut self, passkey: u32) -> &mut Self {
    self.passkey = passkey;
    self
  }

  pub fn set_io_cap(&mut self, iocap: enums::SecurityIOCap) -> &mut Self {
    unsafe { esp_idf_sys::ble_hs_cfg.sm_io_cap = iocap as _ };
    self
  }

  /// If we are the initiator of the security procedure this sets the keys we will distribute.
  pub fn set_security_init_key(&mut self, init_key: enums::PairKeyDist) -> &mut Self {
    unsafe { esp_idf_sys::ble_hs_cfg.sm_our_key_dist = init_key.bits() };
    self
  }

  /// Set the keys we are willing to accept during pairing.
  pub fn set_security_resp_key(&mut self, resp_key: enums::PairKeyDist) -> &mut Self {
    unsafe { esp_idf_sys::ble_hs_cfg.sm_their_key_dist = resp_key.bits() };
    self
  }
}
