use cosmwasm_std::Addr;
use cw_storage_plus::Item;

pub const TARGET: Item<Addr> = Item::new("target");
