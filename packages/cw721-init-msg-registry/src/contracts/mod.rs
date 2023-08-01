#[cfg(feature = "cw721-base")]
pub mod cw721;
#[cfg(feature = "cw721-base")]
pub use cw721::ics721_get_init_msg;

#[cfg(feature = "sg721-base")]
pub mod sg721;
#[cfg(feature = "sg721-base")]
pub use cw721::ics721_get_init_msg;
