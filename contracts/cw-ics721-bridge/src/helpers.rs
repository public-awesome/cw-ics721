use cosmwasm_std::{Addr, Deps, Empty, StdResult};
use cw721::{NftInfoResponse, OwnerOfResponse};

use crate::{
    msg::{ClassIdInfoResponse, GetClassIdForNftContractResponse, GetUriResponse},
    state::{CLASS_ID_TO_CLASS_URI, CLASS_ID_TO_NFT_CONTRACT, NFT_CONTRACT_TO_CLASS_ID},
};

pub const INSTANTIATE_CW721_REPLY_ID: u64 = 4;
pub const ACK_AND_DO_NOTHING: u64 = 0;

pub fn get_owner(deps: Deps, class_id: String, token_id: String) -> StdResult<OwnerOfResponse> {
    let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;
    let resp: OwnerOfResponse = deps.querier.query_wasm_smart(
        class_uri,
        &cw721::Cw721QueryMsg::OwnerOf {
            token_id,
            include_expired: None,
        },
    )?;
    Ok(resp)
}

pub fn get_nft(
    deps: Deps,
    class_id: String,
    token_id: String,
) -> StdResult<NftInfoResponse<Empty>> {
    let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;
    let resp: NftInfoResponse<Empty> = deps
        .querier
        .query_wasm_smart(class_uri, &cw721_base::QueryMsg::NftInfo { token_id })?;
    Ok(resp)
}

pub fn has_class(deps: Deps, class_id: String) -> StdResult<bool> {
    Ok(CLASS_ID_TO_NFT_CONTRACT.has(deps.storage, class_id))
}

pub fn get_class(deps: Deps, class_id: String) -> StdResult<Addr> {
    CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)
}

pub fn get_uri(deps: Deps, class_id: String) -> StdResult<GetUriResponse> {
    let uri = CLASS_ID_TO_CLASS_URI.load(deps.storage, class_id)?;
    Ok(GetUriResponse { uri })
}

pub fn get_class_id_for_nft_contract(
    deps: Deps,
    contract: String,
) -> StdResult<GetClassIdForNftContractResponse> {
    Ok(GetClassIdForNftContractResponse {
        class_id: NFT_CONTRACT_TO_CLASS_ID
            .may_load(deps.storage, deps.api.addr_validate(&contract)?)?,
    })
}

pub fn list_class_ids(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<ClassIdInfoResponse>> {
    let channels = cw_paginate::paginate_map(
        deps,
        &CLASS_ID_TO_NFT_CONTRACT,
        start_after,
        limit,
        cosmwasm_std::Order::Ascending,
    )?;
    Ok(channels
        .into_iter()
        .map(|(class_id, cw721_addr)| ClassIdInfoResponse {
            class_id,
            cw721_addr: cw721_addr.into_string(),
        })
        .collect())
}
