mod contracts;
mod data;

// IMPORTANT: Make sure to remove default features and only enable the wanted feature
pub use contracts::ics721_get_init_msg;
pub use data::InitMsgData;
