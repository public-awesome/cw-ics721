use cosmwasm_std::Addr;
use cw_storage_plus::Item;

use crate::msg::AckMode;

pub const ACK_MODE: Item<AckMode> = Item::new("ack_mode");
pub const LAST_ACK: Item<AckMode> = Item::new("ack_mode");
pub const ICS721: Item<Addr> = Item::new("ics721");
pub const SENT_CALLBACK: Item<Option<cw721::OwnerOfResponse>> = Item::new("sent");
pub const RECEIVED_CALLBACK: Item<Option<cw721::OwnerOfResponse>> = Item::new("received");
