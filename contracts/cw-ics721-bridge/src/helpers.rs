use cosmwasm_std::{to_binary, Addr, Deps, DepsMut, Empty, StdResult, SubMsg, WasmMsg};
use cw721::{NftInfoResponse, OwnerOfResponse};

use crate::{
    msg::{ChannelInfoResponse, ClassIdInfoResponse, GetUriResponse},
    state::{CHANNELS, CLASS_ID_TO_CLASS_URI, CLASS_ID_TO_NFT_CONTRACT},
    ContractError,
};

pub const MINT_SUB_MSG_REPLY_ID: u64 = 0;
pub const TRANSFER_SUB_MSG_REPLY_ID: u64 = 1;
pub const BURN_SUB_MSG_REPLY_ID: u64 = 2;
pub const INSTANTIATE_AND_MINT_CW721_REPLY_ID: u64 = 3;
pub const INSTANTIATE_CW721_REPLY_ID: u64 = 4;
pub const INSTANTIATE_ESCROW_REPLY_ID: u64 = 5;
pub const FAILURE_RESPONSE_FAILURE_REPLY_ID: u64 = 6;
pub const BATCH_TRANSFER_FROM_CHANNEL_REPLY_ID: u64 = 7;
pub const BURN_ESCROW_TOKENS_REPLY_ID: u64 = 8;

pub fn mint(
    deps: DepsMut,
    class_id: String,
    token_id: String,
    token_uri: String,
    receiver: String,
) -> Result<SubMsg, ContractError> {
    if !CLASS_ID_TO_NFT_CONTRACT.has(deps.storage, class_id.clone()) {
        return Err(ContractError::UnrecognisedClassId {});
    }

    let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;
    let mint_msg = cw721_base::ExecuteMsg::<Empty>::Mint(cw721_base::MintMsg::<Empty> {
        token_id,
        owner: receiver,
        token_uri: Some(token_uri),
        extension: Empty {},
    });
    let msg = WasmMsg::Execute {
        contract_addr: class_uri.to_string(),
        msg: to_binary(&mint_msg)?,
        funds: vec![],
    };
    let msg = SubMsg::reply_always(msg, MINT_SUB_MSG_REPLY_ID);

    Ok(msg)
}

pub fn transfer(
    deps: Deps,
    class_id: String,
    token_id: String,
    receiver: String,
) -> Result<SubMsg, ContractError> {
    if !CLASS_ID_TO_NFT_CONTRACT.has(deps.storage, class_id.clone()) {
        return Err(ContractError::UnrecognisedClassId {});
    }
    // Validate receiver
    deps.api.addr_validate(&receiver)?;

    // No need to perform other checks as we can piggyback on cw721-base
    // erroring for us

    let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;
    let transfer_msg = cw721_base::ExecuteMsg::<Empty>::TransferNft {
        recipient: receiver,
        token_id,
    };
    let msg = WasmMsg::Execute {
        contract_addr: class_uri.to_string(),
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    };

    let msg = SubMsg::reply_always(msg, TRANSFER_SUB_MSG_REPLY_ID);
    Ok(msg)
}

pub fn burn(deps: Deps, class_id: String, token_id: String) -> Result<SubMsg, ContractError> {
    if !CLASS_ID_TO_NFT_CONTRACT.has(deps.storage, class_id.clone()) {
        return Err(ContractError::UnrecognisedClassId {});
    }

    // cw721 does not have a burn method by default. That is OK here
    // though as the only way that an address enters the
    // `CLASS_ID_TO_NFT_CONTRACT` map is if it is a cw721 we have
    // instantiated. The cw721s we instantiate have a burn method.

    let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;
    let burn_msg = cw721_base::ExecuteMsg::<Empty>::Burn { token_id };
    let msg = WasmMsg::Execute {
        contract_addr: class_uri.to_string(),
        msg: to_binary(&burn_msg)?,
        funds: vec![],
    };

    let msg = SubMsg::reply_always(msg, BURN_SUB_MSG_REPLY_ID);
    Ok(msg)
}

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

pub fn list_channels(
    deps: Deps,
    start_after: Option<String>,
    limit: Option<u32>,
) -> StdResult<Vec<ChannelInfoResponse>> {
    let channels = cw_paginate::paginate_map(
        deps,
        &CHANNELS,
        start_after,
        limit,
        cosmwasm_std::Order::Ascending,
    )?;
    Ok(channels
        .into_iter()
        .map(|(channel_id, escrow_addr)| ChannelInfoResponse {
            channel_id,
            escrow_addr: escrow_addr.into_string(),
        })
        .collect())
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
