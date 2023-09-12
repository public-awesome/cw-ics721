use cosmwasm_schema::cw_serde;
use cosmwasm_std::ContractInfoResponse;
use sg721_base::msg::CollectionInfoResponse;

/// Collection data provided by the (source) cw721 contract. This is pass as optional class data during interchain transfer to target chain.
/// ICS721 on target chain is free to use this data or not. Lik in case of `sg721-base` it uses owner for defining creator in collection info.
#[cw_serde]
pub struct SgCollectionData {
    // CW721 specific props, copied from ics721::state::CollectionData
    pub owner: Option<String>,
    pub contract_info: ContractInfoResponse,
    pub name: String,
    pub symbol: String,
    pub num_tokens: u64,
    /// SG721 specific collection info
    pub collection_info: CollectionInfoResponse,
}

#[derive(Default)]
pub struct SgIcs721Contract {}
