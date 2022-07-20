use cosmwasm_std::Addr;
use cw_storage_plus::Item;

/// Admin which can withdraw the NFTs, this will be the IBC
/// bridge contract.
pub const ADMIN_ADDRESS: Item<Addr> = Item::new("admin_address");
