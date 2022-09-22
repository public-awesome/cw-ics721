use cw_storage_plus::Item;

use crate::msg::AckMode;

pub const ACK_MODE: Item<AckMode> = Item::new("ack_mode");
