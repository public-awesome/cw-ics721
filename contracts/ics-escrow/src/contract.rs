#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::ADMIN_ADDRESS;

const CONTRACT_NAME: &str = "crates.io:ics-escrow";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let admin_address = deps.api.addr_validate(&msg.admin_address)?;
    ADMIN_ADDRESS.save(deps.storage, &admin_address)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin_address", admin_address.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Withdraw {
            class_uri,
            token_id,
            receiver,
        } => execute_withdraw(deps, env, info, class_uri, token_id, receiver),
    }
}

fn execute_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    class_uri: String,
    token_id: String,
    receiver: String,
) -> Result<Response, ContractError> {
    let admin_address = ADMIN_ADDRESS.load(deps.storage)?;
    if info.sender != admin_address {
        return Err(ContractError::Unauthorized {});
    }

    // Validate receiver and class_uri
    deps.api.addr_validate(&class_uri)?;
    deps.api.addr_validate(&receiver)?;

    let transfer_msg = cw721::Cw721ExecuteMsg::TransferNft {
        recipient: receiver,
        token_id,
    };
    let msg = WasmMsg::Execute {
        contract_addr: class_uri,
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    };
    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_message(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {}
}
