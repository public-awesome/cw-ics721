use cosmwasm_std::entry_point;
use cosmwasm_std::{attr, Response, StdError};
#[cfg(not(feature = "library"))]
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult};
use cw721_base_ibc::msg::{ExecuteMsg, InstantiateMsg, MintMsg, QueryMsg};
use cw721_base_ibc::{ContractError, Cw721Contract};

use cw721_ibc::{Cw721Execute, Cw721Query, NftInfoResponse, OwnerOfResponse};

use crate::state::CLASS_STORAGE;

pub type CW721ContractWrapper<'a> = Cw721Contract<'a, Empty, Empty>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<cosmwasm_std::Response> {
    CW721ContractWrapper::default().instantiate(deps, _env, _info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<Empty>,
) -> Result<Response<Empty>, ContractError> {
    match msg {
        ExecuteMsg::SaveClass {
            class_id,
            class_uri,
        } => save_class(deps, class_id, class_uri),
        _ => CW721ContractWrapper::default().execute(deps, env, info, msg),
    }
}

pub fn save_class(
    deps: DepsMut,
    class_id: String,
    class_uri: String,
) -> Result<Response<Empty>, ContractError> {
    CLASS_STORAGE.save(deps.storage, &class_id, &class_uri)?;
    Ok(Response::default().add_attributes(vec![
        attr("action", "save_class"),
        attr("class_id", class_id),
        attr("class_uri", class_uri),
    ]))
}

pub fn transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    class_id: String,
    token_id: String,
) -> Result<cosmwasm_std::Response, ContractError> {
    CW721ContractWrapper::default().transfer_nft(deps, env, info, recipient, class_id, token_id)
}

pub fn mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
    token_uri: String,
    receiver: String,
) -> Result<cosmwasm_std::Response, ContractError> {
    let mint_msg = MintMsg {
        class_id,
        token_id,
        owner: receiver,
        token_uri: Some(token_uri),
        extension: Empty {},
    };

    CW721ContractWrapper::default().mint(deps, _env, info, mint_msg)
}

pub fn burn(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
) -> Result<cosmwasm_std::Response, ContractError> {
    CW721ContractWrapper::default().burn(deps, _env, info, class_id, token_id)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::OwnerOf {
            class_id,
            token_id,
            include_expired,
        } => to_binary(&get_owner(
            deps,
            _env,
            class_id,
            token_id,
            include_expired.unwrap_or(false),
        )?),
        QueryMsg::NftInfo { class_id, token_id } => to_binary(&get_nft(deps, class_id, token_id)?),
        QueryMsg::HasClass { class_id } => to_binary(&has_class(deps, class_id)),
        QueryMsg::GetClass { class_id } => to_binary(&get_class(deps, class_id)?),
        _ => Err(StdError::GenericErr {
            msg: "Unsupported message type".to_string(),
        }),
    }
}

pub fn get_owner(
    deps: Deps,
    env: Env,
    class_id: String,
    token_id: String,
    include_expired: bool,
) -> StdResult<OwnerOfResponse> {
    CW721ContractWrapper::default().owner_of(deps, env, class_id, token_id, include_expired)
}

pub fn get_nft(
    deps: Deps,
    class_id: String,
    token_id: String,
) -> StdResult<NftInfoResponse<Empty>> {
    CW721ContractWrapper::default().nft_info(deps, class_id, token_id)
}

pub fn has_class(deps: Deps, class_id: String) -> bool {
    CLASS_STORAGE.has(deps.storage, &class_id)
}

pub fn get_class(deps: Deps, class_id: String) -> StdResult<(String, String)> {
    match CLASS_STORAGE.load(deps.storage, &class_id.clone()) {
        Ok(class_uri) => Ok((class_id, class_uri)),
        Err(_) => Err(StdError::generic_err(format!(
            "Class {} not found",
            class_id
        ))),
    }
}
