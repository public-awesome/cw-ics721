#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, IbcMsg, IbcTimeout, MessageInfo, Response, StdResult,
};
use cw2::set_contract_version;
use ics721::ibc::NonFungibleTokenPacketData;

use crate::{
    error::ContractError,
    msg::{AckMode, ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{ACK_MODE, LAST_ACK},
};

const CONTRACT_NAME: &str = "crates.io:ics721-base-tester";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    ACK_MODE.save(deps.storage, &msg.ack_mode)?;
    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::SendPacket {
            channel_id,
            timeout,
            data,
        } => execute_send_packet(channel_id, timeout, data),
        ExecuteMsg::CloseChannel { channel_id } => Ok(execute_close_channel(channel_id)),
        ExecuteMsg::SetAckMode { ack_mode } => execute_set_ack_mode(deps, ack_mode),
    }
}

fn execute_send_packet(
    channel_id: String,
    timeout: IbcTimeout,
    data: NonFungibleTokenPacketData,
) -> Result<Response, ContractError> {
    Ok(Response::default()
        .add_attribute("method", "send_packet")
        .add_message(IbcMsg::SendPacket {
            channel_id,
            data: to_binary(&data)?,
            timeout,
        }))
}

fn execute_close_channel(channel_id: String) -> Response {
    Response::default().add_message(IbcMsg::CloseChannel { channel_id })
}

fn execute_set_ack_mode(deps: DepsMut, ack_mode: AckMode) -> Result<Response, ContractError> {
    ACK_MODE.save(deps.storage, &ack_mode)?;
    Ok(Response::default().add_attribute("method", "set_ack_mode"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::AckMode {} => to_binary(&ACK_MODE.load(deps.storage)?),
        QueryMsg::LastAck {} => to_binary(&LAST_ACK.load(deps.storage)?),
    }
}
