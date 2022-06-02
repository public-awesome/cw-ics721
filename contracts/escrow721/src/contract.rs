use std::error::Error;

use cosmwasm_std::StdError;
#[cfg(not(feature = "library"))]
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult};
use cw721_base_ibc::msg::{InstantiateMsg, MintMsg, QueryMsg};
use cw721_base_ibc::{ContractError, Cw721Contract};
use cw721_ibc::{Cw721Execute, Cw721Query, OwnerOfResponse};

pub type CW721ContractWrapper<'a> = Cw721Contract<'a, Empty, Empty>;

pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<cosmwasm_std::Response> {
    CW721ContractWrapper::default().instantiate(deps, _env, _info, msg)
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
    receiver: String
) -> Result<cosmwasm_std::Response, ContractError> {
    let mint_msg = MintMsg {
        class_id,
        token_id,
        owner: receiver,
        token_uri: Some(token_uri), 
        extension: Empty {}
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
    CW721ContractWrapper::default().burn(
        deps, _env, info, class_id, token_id)
}

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
        _ => CW721ContractWrapper::default().query(deps, _env, msg),
    }
}

pub fn get_owner(
    deps: Deps,
    env: Env,
    class_id: String,
    token_id: String,
    include_expired: bool,
) -> StdResult<OwnerOfResponse> {
    // CW721ContractWrapper::default().owner_of(deps, env, class_id, token_id, include_expired)
    match include_expired {
        true => Ok(OwnerOfResponse {
            owner: "abc123".to_string(),
            approvals: vec![]
        }), 
        false => Err(StdError::GenericErr { msg: "abc123".to_string() } )
    }

}
