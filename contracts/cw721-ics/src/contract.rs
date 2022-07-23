#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw721::Cw721ReceiveMsg;

use crate::ContractError;
use cw721_base::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cw721_base::state::TokenInfo;
use cw721_base::Cw721Contract;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:cw721-ics";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub type CW721ContractWrapper<'a> = Cw721Contract<'a, Empty, Empty>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    // TODO: Do we need any custom logic here. Maybe we want to store class_id
    let res = CW721ContractWrapper::default().instantiate(deps.branch(), env, info, msg)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ok(res)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<Empty>,
) -> Result<Response, ContractError> {
    let tract = CW721ContractWrapper::default();
    match msg {
        // TODO: Add mint route as will probably need some custom logic!
        // TODO: Add burn route as will also need some custom logic!
        ExecuteMsg::TransferNft {
            recipient,
            token_id,
        } => execute_transfer(tract, deps, env, info, recipient, token_id),
        ExecuteMsg::SendNft {
            contract,
            token_id,
            msg,
        } => execute_send(tract, deps, env, info, contract, token_id, msg),
        _ => tract
            .execute(deps, env, info, msg)
            .map_err(ContractError::Cw721),
    }
}

fn check_can_send(
    tract: &CW721ContractWrapper,
    deps: Deps,
    env: &Env,
    info: &MessageInfo,
    token: &TokenInfo<Empty>,
) -> Result<(), ContractError> {
    // Owner or minter (ICS contract) can send
    let minter = tract.minter.load(deps.storage)?;
    if token.owner == info.sender || info.sender == minter {
        return Ok(());
    }

    // any non-expired token approval can send
    if token
        .approvals
        .iter()
        .any(|apr| apr.spender == info.sender && !apr.is_expired(&env.block))
    {
        return Ok(());
    }

    // operator can send
    let op = tract
        .operators
        .may_load(deps.storage, (&token.owner, &info.sender))?;
    match op {
        Some(ex) => {
            if ex.is_expired(&env.block) {
                Err(ContractError::Unauthorized {})
            } else {
                Ok(())
            }
        }
        None => Err(ContractError::Unauthorized {}),
    }
}

fn transfer(
    tract: CW721ContractWrapper,
    deps: DepsMut,
    env: Env,
    info: &MessageInfo,
    recipient: &str,
    token_id: &str,
) -> Result<TokenInfo<Empty>, ContractError> {
    let mut token = tract.tokens.load(deps.storage, token_id)?;
    check_can_send(&tract, deps.as_ref(), &env, info, &token)?;

    token.owner = deps.api.addr_validate(recipient)?;
    token.approvals = vec![];
    tract.tokens.save(deps.storage, token_id, &token)?;
    Ok(token)
}

fn execute_transfer(
    tract: CW721ContractWrapper,
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    token_id: String,
) -> Result<Response, ContractError> {
    // Do the transfer
    transfer(tract, deps, env, &info, &recipient, &token_id)?;

    Ok(Response::new()
        .add_attribute("action", "transfer_nft")
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", recipient)
        .add_attribute("token_id", token_id))
}

fn execute_send(
    tract: CW721ContractWrapper,
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    contract: String,
    token_id: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    // Do the transfer
    transfer(tract, deps, env, &info, &contract, &token_id)?;

    // Build the message to send
    let send = Cw721ReceiveMsg {
        sender: info.sender.to_string(),
        token_id: token_id.clone(),
        msg,
    };

    // Send message
    Ok(Response::new()
        .add_message(send.into_cosmos_msg(contract.clone())?)
        .add_attribute("action", "send_nft")
        .add_attribute("sender", info.sender)
        .add_attribute("recipient", contract)
        .add_attribute("token_id", token_id))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    // TODO: Do we need any custom routes
    CW721ContractWrapper::default().query(deps, env, msg)
}
