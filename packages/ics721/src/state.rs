use cosmwasm_std::{Addr, Binary, Empty};
use cw_pause_once::PauseOrchestrator;
use cw_storage_plus::{Item, Map};
use serde::Deserialize;

use crate::token_types::{Class, ClassId, TokenId};

pub struct Ics721Contract<'a> {
    /// The proxy that this contract is receiving NFTs from, if any.
    pub proxy: Item<'a, Option<Addr>>,
    /// Manages contract pauses.
    pub po: PauseOrchestrator<'a>,
    /// cw721 related info like code ID and token metadata.
    pub cw721_info: Cw721Info<'a>,
    /// cw721 class related info like class ID to cw721 contract mappings.
    pub class_id_info: ClassIdInfo<'a>,
    /// Maps (class ID, token ID) -> to local incoming and outgoing channel ID.
    pub channels_info: ChannelsInfo<'a>,
}

pub struct Cw721Info<'a> {
    /// The code ID we will use for instantiating new cw721s.
    pub cw721_code_id: Item<'a, u64>,
    /// Maps (class ID, token ID) -> token metadata. Used to store
    /// on-chain metadata for tokens that have arrived from other
    /// chains. When a token arrives, it's metadata (regardless of if it
    /// is `None`) is stored in this map. When the token is returned to
    /// it's source chain, the metadata is removed from the map.
    pub token_metadata: Map<'a, (ClassId, TokenId), Option<Binary>>,
}

pub struct ClassIdInfo<'a> {
    /// Maps classID (from NonFungibleTokenPacketData) to the cw721
    /// contract we have instantiated for that classID.
    pub class_id_to_nft_contract: Map<'a, ClassId, Addr>,
    /// Maps cw721 contracts to the classID they were instantiated for.
    pub nft_contract_to_class_id: Map<'a, Addr, ClassId>,

    /// Maps between classIDs and classs. We need to keep this state
    /// ourselves as cw721 contracts do not have class-level metadata.
    pub class_id_to_class: Map<'a, ClassId, Class>,
}

pub struct ChannelsInfo<'a> {
    /// Maps (class ID, token ID) -> local channel ID. Used to determine
    /// the local channel that NFTs have been sent out on.
    pub outgoing_class_token_to_channel: Map<'a, (ClassId, TokenId), String>,
    /// Same as above, but for NFTs arriving at this contract.
    pub incoming_class_token_to_channel: Map<'a, (ClassId, TokenId), String>,
}

impl Default for Cw721Info<'static> {
    fn default() -> Self {
        Self {
            cw721_code_id: Item::new("a"),
            token_metadata: Map::new("j"),
        }
    }
}

impl Default for ClassIdInfo<'static> {
    fn default() -> Self {
        Self {
            class_id_to_nft_contract: Map::new("e"),
            nft_contract_to_class_id: Map::new("f"),
            class_id_to_class: Map::new("g"),
        }
    }
}

impl Default for ChannelsInfo<'static> {
    fn default() -> Self {
        Self {
            outgoing_class_token_to_channel: Map::new("h"),
            incoming_class_token_to_channel: Map::new("i"),
        }
    }
}

impl Default for Ics721Contract<'static> {
    fn default() -> Self {
        Self {
            proxy: Item::new("b"),
            po: PauseOrchestrator::new("c", "d"),
            cw721_info: Cw721Info::default(),
            class_id_info: ClassIdInfo::default(),
            channels_info: ChannelsInfo::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct UniversalAllNftInfoResponse {
    pub access: UniversalOwnerOfResponse,
    pub info: UniversalNftInfoResponse,
}

#[derive(Deserialize)]
pub struct UniversalNftInfoResponse {
    pub token_uri: Option<String>,

    #[serde(skip_deserializing)]
    #[allow(dead_code)]
    extension: Empty,
}

#[derive(Deserialize)]
pub struct UniversalOwnerOfResponse {
    pub owner: String,

    #[serde(skip_deserializing)]
    #[allow(dead_code)]
    pub approvals: Vec<Empty>,
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{from_binary, to_binary, Coin, Empty};

    use super::UniversalAllNftInfoResponse;

    #[test]
    fn test_universal_deserialize() {
        let start = cw721::AllNftInfoResponse::<Coin> {
            access: cw721::OwnerOfResponse {
                owner: "foo".to_string(),
                approvals: vec![],
            },
            info: cw721::NftInfoResponse {
                token_uri: None,
                extension: Coin::new(100, "ujuno"),
            },
        };
        let start = to_binary(&start).unwrap();
        let end: UniversalAllNftInfoResponse = from_binary(&start).unwrap();
        assert_eq!(end.access.owner, "foo".to_string());
        assert_eq!(end.access.approvals, vec![]);
        assert_eq!(end.info.token_uri, None);
        assert_eq!(end.info.extension, Empty::default())
    }
}
