use cosmwasm_schema::{cw_serde, schemars::JsonSchema};
use cosmwasm_std::{Addr, Binary, ContractInfoResponse, Empty};
use cw_pause_once::PauseOrchestrator;
use cw_storage_plus::{Index, IndexList, IndexedMap, Item, Map, UniqueIndex};
use serde::{Deserialize, Serialize};

use ics721_types::token_types::{Class, ClassId, TokenId};

/// The code ID we will use for instantiating new cw721s.
pub const CW721_CODE_ID: Item<u64> = Item::new("a");

/// The incoming proxy that this contract is handling incoming IbcPackets from, if any.
pub const INCOMING_PROXY: Item<Option<Addr>> = Item::new("k");
/// The outgoing proxy that this contract is receiving NFTs from, if any.
pub const OUTGOING_PROXY: Item<Option<Addr>> = Item::new("b");

/// Manages contract pauses.
pub const PO: PauseOrchestrator = PauseOrchestrator::new("c", "d");

/// Maps classID (from NonFungibleTokenPacketData) to the cw721
/// contract we have instantiated for that classID.
/// NOTE: legacy stores with keys `e` and `f` are no longer used.
pub const CLASS_ID_AND_NFT_CONTRACT_INFO: IndexedMap<&str, ClassIdInfo, ClassIdInfoIndexes> =
    IndexedMap::new(
        "m",
        ClassIdInfoIndexes {
            class_id: UniqueIndex::new(|d| d.class_id.clone(), "class_id_info__class_id"),
            address: UniqueIndex::new(|d| d.address.clone(), "class_id_info__address"),
        },
    );

/// Maps between classIDs and classs. We need to keep this state
/// ourselves as cw721 contracts do not have class-level metadata.
pub const CLASS_ID_TO_CLASS: Map<ClassId, Class> = Map::new("g");

/// Maps (class ID, token ID) -> local channel ID. Used to determine
/// the local channel that NFTs have been sent out on.
pub const OUTGOING_CLASS_TOKEN_TO_CHANNEL: Map<(ClassId, TokenId), String> = Map::new("h");
/// Same as above, but for NFTs arriving at this contract.
pub const INCOMING_CLASS_TOKEN_TO_CHANNEL: Map<(ClassId, TokenId), String> = Map::new("i");

/// Maps (class ID, token ID) -> token metadata. Used to store
/// on-chain metadata for tokens that have arrived from other
/// chains. When a token arrives, it's metadata (regardless of if it
/// is `None`) is stored in this map. When the token is returned to
/// it's source chain, the metadata is removed from the map.
pub const TOKEN_METADATA: Map<(ClassId, TokenId), Option<Binary>> = Map::new("j");

/// The admin address for instantiating new cw721 contracts. In case of None, contract is immutable.
pub const ADMIN_USED_FOR_CW721: Item<Option<Addr>> = Item::new("l");

/// The optional contract address length being used for instantiate2. In case of None, default length is 32 (standard in cosmwasm).
/// So length must be shorter than 32. For example, Injective has 20 length address.
/// Bug: https://github.com/CosmWasm/cosmwasm/issues/2155
pub const CONTRACT_ADDR_LENGTH: Item<u32> = Item::new("n");

#[derive(Deserialize, Debug, PartialEq)]
pub struct UniversalAllNftInfoResponse {
    pub access: UniversalOwnerOfResponse,
    pub info: UniversalNftInfoResponse,
}

/// Based on `cw721::ContractInfoResponse v0.18`
#[derive(Deserialize, Debug, PartialEq)]
pub struct UniversalCollectionInfoResponse {
    pub name: String,
    pub symbol: String,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct UniversalNftInfoResponse {
    pub token_uri: Option<String>,

    #[serde(skip_deserializing)]
    #[allow(dead_code)]
    extension: Empty,
}

/// Collection data send by ICS721 on source chain. It is an optional class data for interchain transfer to target chain.
/// ICS721 on target chain is free to use this data or not. Like in case of `sg721-base` it uses owner for defining creator in collection info.
/// `ics721-base` uses name and symbol for instantiating new cw721 contract.
// NB: Please note cw_serde includes `deny_unknown_fields`: https://github.com/CosmWasm/cosmwasm/blob/v1.5.0/packages/schema-derive/src/cw_serde.rs
// For incoming data, parsing needs to be more lenient/less strict, so we use `serde` directly.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[schemars(crate = "cosmwasm_schema::schemars")]
#[serde(crate = "cosmwasm_schema::serde")]
pub struct CollectionData {
    pub owner: Option<String>,
    pub contract_info: Option<ContractInfoResponse>,
    pub name: String,
    pub symbol: String,
    pub num_tokens: Option<u64>,
}

#[derive(Deserialize, Debug, PartialEq)]
pub struct UniversalOwnerOfResponse {
    pub owner: String,

    #[serde(skip_deserializing)]
    #[allow(dead_code)]
    pub approvals: Vec<Empty>,
}

/// ClassIdInfo is used to store associated ClassId for given collection/cw721 address.
#[cw_serde]
pub struct ClassIdInfo {
    /// Associated class_id for a given collection/CW721 address.
    pub class_id: ClassId,
    /// Associated collection/CW721 address for a given class_id.
    pub address: Addr,
}

pub struct ClassIdInfoIndexes<'a> {
    pub class_id: UniqueIndex<'a, ClassId, ClassIdInfo>,
    pub address: UniqueIndex<'a, Addr, ClassIdInfo>,
}

impl<'a> IndexList<ClassIdInfo> for ClassIdInfoIndexes<'a> {
    fn get_indexes(&'_ self) -> Box<dyn Iterator<Item = &'_ dyn Index<ClassIdInfo>> + '_> {
        let v: Vec<&dyn Index<ClassIdInfo>> = vec![&self.class_id, &self.address];
        Box::new(v.into_iter())
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{from_json, to_json_binary, Coin, Empty};

    use super::UniversalAllNftInfoResponse;

    #[test]
    fn test_universal_deserialize() {
        let start = cw721::msg::AllNftInfoResponse::<Coin> {
            access: cw721::msg::OwnerOfResponse {
                owner: "foo".to_string(),
                approvals: vec![],
            },
            info: cw721::msg::NftInfoResponse {
                token_uri: None,
                extension: Coin::new(100, "ujuno"),
            },
        };
        let start = to_json_binary(&start).unwrap();
        let end: UniversalAllNftInfoResponse = from_json(start).unwrap();
        assert_eq!(end.access.owner, "foo".to_string());
        assert_eq!(end.access.approvals, vec![]);
        assert_eq!(end.info.token_uri, None);
        assert_eq!(end.info.extension, Empty::default())
    }
}
