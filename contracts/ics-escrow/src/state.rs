use cosmwasm_std::Addr;
use cw_storage_plus::Item;

/// Admin which can withdraw the NFTs, this will be the IBC
/// bridge contract.
pub const ADMIN_ADDRESS: Item<Addr> = Item::new("admin_address");

/// Channel we are escrowing tokens for on behalf of the bridge
/// contract.
pub const CHANNEL_ID: Item<String> = Item::new("channel_id");
