use cosmwasm_std::{Addr, Empty};
use cw_storage_plus::Map;

pub const CHANNELS: Map<String, Empty> = Map::new("channels");

/// Class URI for us will be a contract address
pub const CLASS_ID_TO_CLASS_URI: Map<String, Addr> = Map::new("class_id_to_contract");
pub const CLASS_URI_TO_CLASS_ID: Map<Addr, String> = Map::new("contract_to_class_id");
