#[cfg(not(feature = "library"))]
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult};
use cw721_base_ibc::msg::{ExecuteMsg, InstantiateMsg, MintMsg, QueryMsg};
use cw721_base_ibc::{ContractError, Cw721Contract};
use cw721_ibc::{Cw721Execute, Cw721Query, OwnerOfResponse};

pub type CW721ContractWrapper<'a> = Cw721Contract<'a, Empty, Empty>;

pub fn instantiate_entry(
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

pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<Empty>,
) -> Result<cosmwasm_std::Response, ContractError> {
    println!("in the execute");
    match msg {
        ExecuteMsg::Mint(msg) => mint(deps, env, info, msg),
        _ => Err(ContractError::Expired {}),
    }
}

pub fn mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: MintMsg<Empty>,
) -> Result<cosmwasm_std::Response, ContractError> {
    CW721ContractWrapper::default().mint(deps, _env, info, msg)
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
    CW721ContractWrapper::default().owner_of(deps, env, class_id, token_id, include_expired)
}
