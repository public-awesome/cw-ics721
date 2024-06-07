use cosmwasm_std::{
    to_json_binary, Addr, Binary, Deps, Empty, Env, Order, StdError, StdResult, Storage,
};
use cw721::{DefaultOptionalCollectionExtension, DefaultOptionalNftExtension};
use cw_storage_plus::{Bound, Map};
use sha2::{Digest, Sha256};

use crate::{
    helpers::get_instantiate2_address,
    msg::QueryMsg,
    state::{
        UniversalAllNftInfoResponse, ADMIN_USED_FOR_CW721, CLASS_ID_AND_NFT_CONTRACT_INFO,
        CLASS_ID_TO_CLASS, CONTRACT_ADDR_LENGTH, CW721_CODE_ID, INCOMING_CLASS_TOKEN_TO_CHANNEL,
        INCOMING_PROXY, OUTGOING_CLASS_TOKEN_TO_CHANNEL, OUTGOING_PROXY, PO, TOKEN_METADATA,
    },
    ContractError,
};
use ics721_types::token_types::{Class, ClassId, ClassToken, Token, TokenId};

pub trait Ics721Query {
    fn query(&self, deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
        match msg {
            QueryMsg::ClassId { contract } => Ok(to_json_binary(
                &query_class_id_for_nft_contract(deps, contract)?,
            )?),
            QueryMsg::NftContract { class_id } => Ok(to_json_binary(
                &query_nft_contract_for_class_id(deps.storage, class_id.into())?,
            )?),
            QueryMsg::GetInstantiate2NftContract {
                class_id,
                cw721_code_id,
            } => Ok(to_json_binary(&query_get_instantiate2_nft_contract(
                deps,
                &env,
                class_id.into(),
                cw721_code_id,
            )?)?),
            QueryMsg::ClassMetadata { class_id } => {
                Ok(to_json_binary(&query_class_metadata(deps, class_id)?)?)
            }
            QueryMsg::TokenMetadata { class_id, token_id } => Ok(to_json_binary(
                &query_token_metadata(deps, class_id, token_id)?,
            )?),
            QueryMsg::Owner { class_id, token_id } => {
                Ok(to_json_binary(&query_owner(deps, class_id, token_id)?)?)
            }
            QueryMsg::Pauser {} => Ok(to_json_binary(&PO.query_pauser(deps.storage)?)?),
            QueryMsg::Paused {} => Ok(to_json_binary(&PO.query_paused(deps.storage)?)?),
            QueryMsg::OutgoingProxy {} => Ok(to_json_binary(&OUTGOING_PROXY.load(deps.storage)?)?),
            QueryMsg::IncomingProxy {} => Ok(to_json_binary(&INCOMING_PROXY.load(deps.storage)?)?),
            QueryMsg::Cw721CodeId {} => Ok(to_json_binary(&query_cw721_code_id(deps)?)?),
            QueryMsg::Cw721Admin {} => {
                Ok(to_json_binary(&ADMIN_USED_FOR_CW721.load(deps.storage)?)?)
            }
            QueryMsg::ContractAddrLength {} => Ok(to_json_binary(
                &CONTRACT_ADDR_LENGTH.may_load(deps.storage)?,
            )?),
            QueryMsg::NftContracts { start_after, limit } => Ok(to_json_binary(
                &query_nft_contracts(deps, start_after, limit)?,
            )?),
            QueryMsg::OutgoingChannels { start_after, limit } => Ok(to_json_binary(
                &query_channels(deps, &OUTGOING_CLASS_TOKEN_TO_CHANNEL, start_after, limit)?,
            )?),
            QueryMsg::IncomingChannels { start_after, limit } => Ok(to_json_binary(
                &query_channels(deps, &INCOMING_CLASS_TOKEN_TO_CHANNEL, start_after, limit)?,
            )?),
        }
    }
}

pub fn query_class_id_for_nft_contract(deps: Deps, contract: String) -> StdResult<Option<ClassId>> {
    let contract = deps.api.addr_validate(&contract)?;
    load_class_id_for_nft_contract(deps.storage, &contract)
}

pub fn load_class_id_for_nft_contract(
    storage: &dyn Storage,
    contract: &Addr,
) -> StdResult<Option<ClassId>> {
    CLASS_ID_AND_NFT_CONTRACT_INFO
        .idx
        .address
        .item(storage, contract.clone())
        .map(|e| e.map(|(_, c)| c.class_id))
}

pub fn query_nft_contract_for_class_id(
    storage: &dyn Storage,
    class_id: ClassId,
) -> StdResult<Option<Addr>> {
    // Query the IndexedMap using the class_id index
    CLASS_ID_AND_NFT_CONTRACT_INFO
        .idx
        .class_id
        .item(storage, class_id)
        .map(|e| e.map(|(_, v)| v.address))
}

/// Returns the address of the instantiated NFT contract for the given class_id.
/// If cw721_code_id is None, it will use the default CW721_CODE_ID.
pub fn query_get_instantiate2_nft_contract(
    deps: Deps,
    env: &Env,
    class_id: ClassId,
    cw721_code_id: Option<u64>,
) -> Result<Addr, ContractError> {
    let cw721_code_id = if let Some(cw721_code_id) = cw721_code_id {
        cw721_code_id
    } else {
        CW721_CODE_ID.load(deps.storage)?
    };

    let mut hasher = Sha256::new();
    hasher.update(class_id.as_bytes());
    let salt = hasher.finalize().to_vec();

    let nft_contract =
        get_instantiate2_address(deps, env.contract.address.as_str(), &salt, cw721_code_id)?;
    Ok(nft_contract)
}

pub fn load_nft_contract_for_class_id(storage: &dyn Storage, class_id: String) -> StdResult<Addr> {
    query_nft_contract_for_class_id(storage, class_id.clone().into())?.map_or_else(
        || {
            Err(StdError::NotFound {
                kind: format!("NFT contract not found for class id {}", class_id),
            })
        },
        Ok,
    )
}

pub fn query_class_metadata(deps: Deps, class_id: String) -> StdResult<Option<Class>> {
    CLASS_ID_TO_CLASS.may_load(deps.storage, ClassId::new(class_id))
}

pub fn query_token_metadata(
    deps: Deps,
    class_id: String,
    token_id: String,
) -> StdResult<Option<Token>> {
    let token_id = TokenId::new(token_id);
    let class_id = ClassId::new(class_id);

    let Some(token_metadata) =
        TOKEN_METADATA.may_load(deps.storage, (class_id.clone(), token_id.clone()))?
    else {
        // Token metadata is set unconditionaly on mint. If we have no
        // metadata entry, we have no entry for this token at all.
        return Ok(None);
    };
    let Some(nft_contract) = query_nft_contract_for_class_id(deps.storage, class_id)? else {
        debug_assert!(false, "token_metadata != None => token_contract != None");
        return Ok(None);
    };
    let UniversalAllNftInfoResponse { info, .. } = deps.querier.query_wasm_smart(
        nft_contract,
        &cw721::msg::Cw721QueryMsg::<
            DefaultOptionalNftExtension,
            DefaultOptionalCollectionExtension,
            Empty,
        >::AllNftInfo {
            token_id: token_id.clone().into(),
            include_expired: None,
        },
    )?;
    Ok(Some(Token {
        id: token_id,
        uri: info.token_uri,
        data: token_metadata,
    }))
}

pub fn query_owner(
    deps: Deps,
    class_id: String,
    token_id: String,
) -> StdResult<cw721::msg::OwnerOfResponse> {
    let nft_contract = load_nft_contract_for_class_id(deps.storage, class_id)?;
    let resp: cw721::msg::OwnerOfResponse = deps.querier.query_wasm_smart(
        nft_contract,
        &cw721::msg::Cw721QueryMsg::<
            DefaultOptionalNftExtension,
            DefaultOptionalCollectionExtension,
            Empty,
        >::OwnerOf {
            token_id,
            include_expired: None,
        },
    )?;
    Ok(resp)
}

pub fn query_cw721_code_id(deps: Deps) -> StdResult<u64> {
    CW721_CODE_ID.load(deps.storage)
}

pub fn query_nft_contracts(
    deps: Deps,
    start_after: Option<ClassId>,
    limit: Option<u32>,
) -> StdResult<Vec<(String, Addr)>> {
    let start = start_after.map(|s| Bound::ExclusiveRaw(s.to_string().into()));
    let all = CLASS_ID_AND_NFT_CONTRACT_INFO
        .range(deps.storage, start, None, Order::Ascending)
        .map(|item| item.map(|(k, v)| (k, v.address)));
    match limit {
        Some(limit) => all.take(limit as usize).collect(),
        None => all.collect(),
    }
}

fn query_channels(
    deps: Deps,
    class_token_to_channel: &Map<(ClassId, TokenId), String>,
    start_after: Option<ClassToken>,
    limit: Option<u32>,
) -> StdResult<Vec<((String, String), String)>> {
    let start_after = start_after.map(|class_token| {
        (
            ClassId::new(class_token.class_id),
            TokenId::new(class_token.token_id),
        )
    });
    cw_paginate_storage::paginate_map(
        deps,
        class_token_to_channel,
        start_after,
        limit,
        Order::Ascending,
    )
}
