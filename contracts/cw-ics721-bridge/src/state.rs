use cosmwasm_std::Empty;
use cw_storage_plus::Map;

pub const CHANNELS: Map<String, Empty> = Map::new("channels");
