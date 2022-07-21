#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{ADMIN_ADDRESS, CHANNEL_ID};

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
    CHANNEL_ID.save(deps.storage, &msg.channel_id)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin_address", admin_address.to_string())
        .add_attribute("channel_id", msg.channel_id))
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
            nft_address,
            token_id,
            receiver,
        } => execute_withdraw(deps, env, info, nft_address, token_id, receiver),
    }
}

fn execute_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    nft_address: String,
    token_id: String,
    receiver: String,
) -> Result<Response, ContractError> {
    let admin_address = ADMIN_ADDRESS.load(deps.storage)?;
    if info.sender != admin_address {
        return Err(ContractError::Unauthorized {});
    }

    // Validate receiver and class_uri
    deps.api.addr_validate(&nft_address)?;
    deps.api.addr_validate(&receiver)?;

    let transfer_msg = cw721::Cw721ExecuteMsg::TransferNft {
        recipient: receiver,
        token_id,
    };
    let msg = WasmMsg::Execute {
        contract_addr: nft_address,
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    };
    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_message(msg))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::AdminAddress {} => to_binary(&ADMIN_ADDRESS.load(deps.storage)?),
        QueryMsg::ChannelId {} => to_binary(&CHANNEL_ID.load(deps.storage)?),
    }
}
