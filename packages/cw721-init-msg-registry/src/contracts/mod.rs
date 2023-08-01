#[cfg(feature = "cw721-base")]
pub mod cw721;

#[cfg(feature = "sg721-base")]
pub mod sg721;

cfg_if::cfg_if! {
  if #[cfg(feature = "sg721-base")] {
    pub use self::sg721::ics721_get_init_msg;
  } else {
    pub use self::cw721::ics721_get_init_msg;
  }
}
