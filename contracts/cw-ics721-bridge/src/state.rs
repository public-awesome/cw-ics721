use cosmwasm_std::{Addr, Binary, Empty};
use cw_pause_once::PauseOrchestrator;
use cw_storage_plus::{Item, Map};
use serde::Deserialize;

use crate::token_types::{Class, ClassId, TokenId};

/// The code ID we will use for instantiating new cw721s.
pub const CW721_CODE_ID: Item<u64> = Item::new("a");
/// The proxy that this contract is receiving NFTs from, if any.
pub const PROXY: Item<Option<Addr>> = Item::new("b");
/// Manages contract pauses.
pub const PO: PauseOrchestrator = PauseOrchestrator::new("c", "d");

/// Maps classID (from NonFungibleTokenPacketData) to the cw721
/// contract we have instantiated for that classID.
pub const CLASS_ID_TO_NFT_CONTRACT: Map<ClassId, Addr> = Map::new("e");
/// Maps cw721 contracts to the classID they were instantiated for.
pub const NFT_CONTRACT_TO_CLASS: Map<Addr, Class> = Map::new("f");

/// Maps between classIDs and classs. We need to keep this state
/// ourselves as cw721 contracts do not have class-level metadata.
pub const CLASS_ID_TO_CLASS: Map<ClassId, Class> = Map::new("g");

/// Maps (class ID, token ID) -> local channel ID. Used to determine
/// the local channel that NFTs have been sent out on.
pub const OUTGOING_CLASS_TOKEN_TO_CHANNEL: Map<(ClassId, TokenId), String> = Map::new("h");
/// Same as above, but for NFTs arriving at this contract.
pub const INCOMING_CLASS_TOKEN_TO_CHANNEL: Map<(ClassId, TokenId), String> = Map::new("i");
/// metadata of a token (class id, token id) -> metadata
pub const CLASS_TOKEN_ID_TO_TOKEN_METADATA: Map<(ClassId, TokenId), Option<Binary>> = Map::new("j");

#[derive(Deserialize)]
pub struct UniversalNftInfoResponse {
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
