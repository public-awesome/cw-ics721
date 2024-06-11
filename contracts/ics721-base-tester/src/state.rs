use cosmwasm_std::Addr;
use cw_storage_plus::Item;

use crate::msg::AckMode;

pub const ACK_MODE: Item<AckMode> = Item::new("ack_mode");
pub const LAST_ACK: Item<AckMode> = Item::new("ack_mode");

pub const ICS721: Item<Addr> = Item::new("ics721");
pub const SENT_CALLBACK: Item<Option<cw721::msg::OwnerOfResponse>> = Item::new("sent");
pub const RECEIVED_CALLBACK: Item<Option<cw721::msg::OwnerOfResponse>> = Item::new("received");
pub const NFT_CONTRACT: Item<Addr> = Item::new("nft_contract");
pub const CW721_RECEIVE: Item<String> = Item::new("cw721_received");
