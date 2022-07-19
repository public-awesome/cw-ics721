#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::helpers::{burn, get_class, get_nft, get_owner, has_class, transfer};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};

const CONTRACT_NAME: &str = "crates.io:cw-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    todo!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer {
            class_id,
            token_id,
            receiver,
        } => execute_transfer(deps, env, info, class_id, token_id, receiver),
        ExecuteMsg::Burn { class_id, token_id } => {
            execute_burn(deps, env, info, class_id, token_id)
        }
    }
}

fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
    receiver: String,
) -> Result<Response, ContractError> {
    // This will error if the class_id does not exist so no need to check
    let owner = get_owner(deps.as_ref(), class_id.clone(), token_id.clone())?;

    // Check if we are the owner or the contract itself
    if info.sender != env.contract.address && info.sender != owner.owner {
        return Err(ContractError::Unauthorized {});
    }

    let msg = transfer(deps, class_id, token_id, receiver)?;
    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_submessage(msg))
}

fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
) -> Result<Response, ContractError> {
    // This will error if the class_id does not exist so no need to check
    let owner = get_owner(deps.as_ref(), class_id.clone(), token_id.clone())?;

    // Check if we are the owner or the contract itself
    if info.sender != env.contract.address && info.sender != owner.owner {
        return Err(ContractError::Unauthorized {});
    }

    let msg = burn(deps, class_id, token_id)?;
    Ok(Response::new()
        .add_attribute("action", "burn")
        .add_submessage(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner { token_id, class_id } => {
            to_binary(&get_owner(deps, class_id, token_id)?)
        }
        QueryMsg::GetNft { class_id, token_id } => to_binary(&get_nft(deps, class_id, token_id)?),
        QueryMsg::HasClass { class_id } => to_binary(&has_class(deps, class_id)?),
        QueryMsg::GetClass { class_id } => to_binary(&get_class(deps, class_id)?),
    }
}
