use cw_storage_plus::Item;

use crate::msg::AckMode;

pub const ACK_MODE: Item<AckMode> = Item::new("ack_mode");
pub const LAST_ACK: Item<AckMode> = Item::new("ack_mode");
