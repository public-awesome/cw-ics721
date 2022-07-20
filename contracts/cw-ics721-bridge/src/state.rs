use cosmwasm_std::{Addr, Empty};
use cw_storage_plus::{Item, Map};

/// Contains the set of channel_ids we currently have connections to.
pub const CHANNELS: Map<String, Empty> = Map::new("channels");

/// The code ID we will use for instantiating new cw721s.
pub const CW721_CODE_ID: Item<u64> = Item::new("cw721_code_id");

// The code ID we will use when instantiating escrow contracts.
pub const ESCROW_CODE_ID: Item<u64> = Item::new("escrow_code_id");

/// Maps classID (from NonFungibleTokenPacketData) to the cw721
/// contract we have instantiated for that classID.
pub const CLASS_ID_TO_NFT_CONTRACT: Map<String, Addr> = Map::new("class_id_to_contract");
/// Maps cw721 contracts to the classID they were instantiated for.
pub const NFT_CONTRACT_TO_CLASS_ID: Map<Addr, String> = Map::new("contract_to_class_id");
