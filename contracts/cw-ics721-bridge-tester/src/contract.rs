#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Env, IbcMsg, IbcTimeout, MessageInfo, Response, StdResult,
    WasmMsg,
};
use cw2::set_contract_version;
use ics721::NonFungibleTokenPacketData;

use crate::{
    error::ContractError,
    msg::{AckMode, ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{ACK_MODE, ICS721, LAST_ACK, RECEIVED_CALLBACK, SENT_CALLBACK},
};

const CONTRACT_NAME: &str = "crates.io:cw-icw721-bridge-tester";
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
    ICS721.save(deps.storage, &deps.api.addr_validate(&msg.ics721)?)?;
    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ReceiveNft(msg) => ics721_cb::handle_receive_callback(deps, msg),
        ExecuteMsg::Ics721Callback(msg) => ics721_cb::handle_callback_callback(deps, msg),
        ExecuteMsg::SendNft {
            cw721,
            ics721,
            token_id,
            recipient,
            channel_id,
            memo,
        } => execute_send_nft(env, cw721, ics721, token_id, recipient, channel_id, memo),
        ExecuteMsg::SendPacket {
            channel_id,
            timeout,
            data,
        } => execute_send_packet(channel_id, timeout, data),
        ExecuteMsg::CloseChannel { channel_id } => Ok(execute_close_channel(channel_id)),
        ExecuteMsg::SetAckMode { ack_mode } => execute_set_ack_mode(deps, ack_mode),
    }
}

mod ics721_cb {
    use cosmwasm_std::{from_binary, Addr, DepsMut, Response};
    use ics721::{Ics721CallbackMsg, Ics721ReceiveMsg, Ics721Status, NonFungibleTokenPacketData};

    use crate::{
        msg::Ics721Callbacks,
        state::{ICS721, RECEIVED_CALLBACK, SENT_CALLBACK},
        ContractError,
    };

    pub(crate) fn handle_receive_callback(
        deps: DepsMut,
        msg: Ics721ReceiveMsg,
    ) -> Result<Response, ContractError> {
        match from_binary::<Ics721Callbacks>(&msg.msg)? {
            Ics721Callbacks::NftReceived {} => nft_received(deps, msg.original_packet),
            Ics721Callbacks::FailCallback {} => fail_callback(),
            _ => Err(ContractError::InvalidCallback {}),
        }
    }

    pub(crate) fn handle_callback_callback(
        deps: DepsMut,
        msg: Ics721CallbackMsg,
    ) -> Result<Response, ContractError> {
        match from_binary::<Ics721Callbacks>(&msg.msg)? {
            Ics721Callbacks::NftSent {} => nft_sent(deps, msg.status, msg.original_packet),
            Ics721Callbacks::FailCallback {} => fail_callback(),
            _ => Err(ContractError::InvalidCallback {}),
        }
    }

    fn nft_sent(
        deps: DepsMut,
        status: Ics721Status,
        packet: NonFungibleTokenPacketData,
    ) -> Result<Response, ContractError> {
        let ics_addr = ICS721.load(deps.storage)?;
        let nft_contract = match deps.querier.query_wasm_smart::<Option<Addr>>(
            ics_addr,
            &cw_ics721_bridge::msg::QueryMsg::NftContract {
                class_id: packet.class_id.to_string(),
            },
        )? {
            Some(addr) => addr,
            None => deps.api.addr_validate(&packet.class_id)?,
        };

        let owner: Option<cw721::OwnerOfResponse> = deps
            .querier
            .query_wasm_smart::<cw721::OwnerOfResponse>(
                nft_contract,
                &cw721::Cw721QueryMsg::OwnerOf {
                    token_id: packet.token_ids[0].clone().into(),
                    include_expired: None,
                },
            )
            .ok();

        SENT_CALLBACK.save(deps.storage, &owner)?;

        match status {
            Ics721Status::Success => {
                // Transfer completed, the owner should either be None
                // or ics721 if we on source chain,
                // the owner should be ics721 if we on
                // dest chain, the owner should be None
            }
            Ics721Status::Failed => {
                // Transfer failed, the NFT owner should be the sender
            }
        }

        Ok(Response::new())
    }

    /// We don't care about the status on receive callback because if
    /// the transfer failed the callback wont be called anyway, so
    /// we assume the transfer is always successful.`
    fn nft_received(
        deps: DepsMut,
        packet: NonFungibleTokenPacketData,
    ) -> Result<Response, ContractError> {
        // Owner should be the receiver.
        let ics_addr = ICS721.load(deps.storage)?;
        let nft_contract = match deps.querier.query_wasm_smart::<Option<Addr>>(
            ics_addr,
            &cw_ics721_bridge::msg::QueryMsg::NftContract {
                class_id: packet.class_id.to_string(),
            },
        )? {
            Some(addr) => addr,
            None => deps.api.addr_validate(&packet.class_id)?,
        };

        let owner = deps
            .querier
            .query_wasm_smart::<cw721::OwnerOfResponse>(
                nft_contract,
                &cw721::Cw721QueryMsg::OwnerOf {
                    token_id: packet.token_ids[0].clone().into(),
                    include_expired: None,
                },
            )
            .ok();

        RECEIVED_CALLBACK.save(deps.storage, &owner)?;

        Ok(Response::new())
    }

    fn fail_callback() -> Result<Response, ContractError> {
        // we want to test what happens when an callback is failed

        // On ACK callback nothing should happen, ack callback is just a
        // notifier to the sending contract that the NFT was
        // transferred successfully or not.

        // On Receive callback it is important if the callback fails or not,
        // because we send the NFT with a purpose to this contract, so if the
        // callback fails it means we didn't get what we wanted, so we
        // should send the NFT back to the sender. but the callback
        // was successful, everything is fine. Ex: marketplaces can
        // accept NFTs and place them on sale, if the `put on sale` process
        // fails the NFT should be sent back to the sender.
        Err(ContractError::RandomError)
    }
}

fn execute_send_nft(
    env: Env,
    cw721: String,
    ics721: String,
    token_id: String,
    recipient: String,
    channel_id: String,
    memo: Option<String>,
) -> Result<Response, ContractError> {
    // Send send msg to cw721, send it to ics721 with the correct msg.
    let msg = WasmMsg::Execute {
        contract_addr: cw721,
        msg: to_binary(&cw721::Cw721ExecuteMsg::SendNft {
            contract: ics721,
            token_id,
            msg: to_binary(&ics721::IbcOutgoingMsg {
                receiver: recipient,
                channel_id,
                timeout: IbcTimeout::with_timestamp(env.block.time.plus_seconds(1000)),
                memo,
            })?,
        })?,
        funds: vec![],
    };

    Ok(Response::default().add_message(msg))
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
        QueryMsg::GetReceivedCallback {} => to_binary(&RECEIVED_CALLBACK.load(deps.storage)?),
        QueryMsg::GetSentCallback {} => to_binary(&SENT_CALLBACK.load(deps.storage)?),
    }
}
