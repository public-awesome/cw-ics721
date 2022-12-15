use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, Empty};
use cw_pause_once::PauseOrchestrator;
use cw_storage_plus::{Item, Map};
use serde::Deserialize;

/// The code ID we will use for instantiating new cw721s.
pub(crate) const CW721_CODE_ID: Item<u64> = Item::new("a");
/// The proxy that this contract is receiving NFTs from, if any.
pub(crate) const PROXY: Item<Option<Addr>> = Item::new("b");
/// Manages contract pauses.
pub(crate) const PO: PauseOrchestrator = PauseOrchestrator::new("c", "d");

/// Maps classID (from NonFungibleTokenPacketData) to the cw721
/// contract we have instantiated for that classID.
pub(crate) const CLASS_ID_TO_NFT_CONTRACT: Map<String, Addr> = Map::new("e");
/// Maps cw721 contracts to the classID they were instantiated for.
pub(crate) const NFT_CONTRACT_TO_CLASS_ID: Map<Addr, String> = Map::new("f");

/// Maps between classIDs and classUris. We need to keep this state
/// ourselves as cw721 contracts do not have class-level metadata. Not
/// all collections have associated metadata so `may_load` should be
/// used when reading from this map.
pub(crate) const CLASS_ID_TO_COLLECTION_INFO: Map<String, CollectionMetadata> = Map::new("g");
/// Maps between a token and it's associated metadata. Not all tokens
/// have on-chain metadata so `may_load` should be used when reading
/// from this map.
pub(crate) const TOKEN_TO_TOKEN_DATA: Map<(String, String), Binary> = Map::new("h");

/// Maps (class ID, token ID) -> local channel ID. Used to determine
/// the local channel that NFTs have been sent out on.
pub(crate) const OUTGOING_CLASS_TOKEN_TO_CHANNEL: Map<(String, String), String> = Map::new("i");
/// Same as above, but for NFTs arriving at this contract.
pub(crate) const INCOMING_CLASS_TOKEN_TO_CHANNEL: Map<(String, String), String> = Map::new("j");

#[cw_serde]
#[derive(Default)]
pub(crate) struct CollectionMetadata {
    pub class_uri: String,
    pub class_data: Binary,
}

#[derive(Deserialize)]
pub(crate) struct UniversalNftInfoResponse {
    pub token_uri: Option<String>,

    #[serde(skip_deserializing)]
    #[allow(dead_code)]
    extension: Empty,
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{from_binary, to_binary, Coin, Empty};

    use super::UniversalNftInfoResponse;

    #[test]
    fn test_universal_deserialize() {
        let start = cw721::NftInfoResponse::<Coin> {
            token_uri: None,
            extension: Coin::new(100, "ujuno"),
        };
        let start = to_binary(&start).unwrap();
        let end: UniversalNftInfoResponse = from_binary(&start).unwrap();
        assert_eq!(end.token_uri, None);
        assert_eq!(end.extension, Empty::default())
    }
}
