use cosmwasm_std::Addr;
use cw_storage_plus::{Item, Map};

// This map is used to store key: class_id to data: class_uri
// per the save_class method in contracts/escrow721/src/contract.rs
pub const CLASS_STORAGE: Map<&str, String> = Map::new("class_storage");
pub const ESCROW_ADDRESSES: Item<Addr> = Item::new("escrow_addresses");
