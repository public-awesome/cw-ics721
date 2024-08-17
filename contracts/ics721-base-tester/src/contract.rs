#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_json_binary, Binary, Deps, DepsMut, Empty, Env, IbcMsg, IbcTimeout, MessageInfo, Response,
    StdResult, WasmMsg,
};
use cw2::set_contract_version;
use cw721::{DefaultOptionalCollectionExtensionMsg, DefaultOptionalNftExtensionMsg};
use ics721_types::ibc_types::{IbcOutgoingMsg, NonFungibleTokenPacketData};

use crate::{
    error::ContractError,
    msg::{AckMode, ExecuteMsg, InstantiateMsg, QueryMsg},
    state::{ACK_MODE, ICS721, LAST_ACK, NFT_CONTRACT, RECEIVED_CALLBACK, SENT_CALLBACK},
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
    ICS721.save(deps.storage, &deps.api.addr_validate(&msg.ics721)?)?;
    Ok(Response::new().add_attribute("method", "instantiate"))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ReceiveNft(msg) => receive_callbacks::handle_receive_cw_callback(deps, msg),
        ExecuteMsg::Ics721ReceiveCallback(msg) => {
            receive_callbacks::handle_receive_callback(deps, &info, msg)
        }
        ExecuteMsg::Ics721AckCallback(msg) => {
            receive_callbacks::handle_ack_callback(deps, &info, msg)
        }
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

mod receive_callbacks {
    use cosmwasm_std::{ensure_eq, from_json, DepsMut, Empty, MessageInfo, Response};
    use cw721::{DefaultOptionalCollectionExtension, DefaultOptionalNftExtension};
    use ics721_types::{
        ibc_types::NonFungibleTokenPacketData,
        types::{Ics721AckCallbackMsg, Ics721ReceiveCallbackMsg, Ics721Status},
    };

    use crate::{
        msg::Ics721Callbacks,
        state::{CW721_RECEIVE, ICS721, NFT_CONTRACT, RECEIVED_CALLBACK, SENT_CALLBACK},
        ContractError,
    };

    pub(crate) fn handle_receive_cw_callback(
        deps: DepsMut,
        _msg: cw721::receiver::Cw721ReceiveMsg,
    ) -> Result<Response, ContractError> {
        // We got the callback, so its working
        CW721_RECEIVE.save(deps.storage, &"success".to_string())?;
        Ok(Response::new())
    }

    pub(crate) fn handle_receive_callback(
        deps: DepsMut,
        info: &MessageInfo,
        msg: Ics721ReceiveCallbackMsg,
    ) -> Result<Response, ContractError> {
        match from_json::<Ics721Callbacks>(&msg.msg)? {
            Ics721Callbacks::NftReceived {} => {
                nft_received(deps, info, msg.original_packet, msg.nft_contract)
            }
            Ics721Callbacks::FailCallback {} => fail_callback(),
            _ => Err(ContractError::InvalidCallback {}),
        }
    }

    pub(crate) fn handle_ack_callback(
        deps: DepsMut,
        info: &MessageInfo,
        msg: Ics721AckCallbackMsg,
    ) -> Result<Response, ContractError> {
        match from_json::<Ics721Callbacks>(&msg.msg)? {
            Ics721Callbacks::NftSent {} => nft_sent(
                deps,
                info,
                msg.status,
                msg.original_packet,
                msg.nft_contract,
            ),
            Ics721Callbacks::FailCallback {} => fail_callback(),
            _ => Err(ContractError::InvalidCallback {}),
        }
    }

    fn nft_sent(
        deps: DepsMut,
        info: &MessageInfo,
        status: Ics721Status,
        packet: NonFungibleTokenPacketData,
        nft_contract: String,
    ) -> Result<Response, ContractError> {
        let ics_addr = ICS721.load(deps.storage)?;
        ensure_eq!(ics_addr, info.sender, ContractError::SenderIsNotIcs721);

        NFT_CONTRACT.save(deps.storage, &deps.api.addr_validate(&nft_contract)?)?;

        let owner: Option<cw721::msg::OwnerOfResponse> = deps
            .querier
            .query_wasm_smart::<cw721::msg::OwnerOfResponse>(
                nft_contract,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
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
            Ics721Status::Failed(..) => {
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
        info: &MessageInfo,
        packet: NonFungibleTokenPacketData,
        nft_contract: String,
    ) -> Result<Response, ContractError> {
        // Owner should be the receiver.
        let ics_addr = ICS721.load(deps.storage)?;
        ensure_eq!(ics_addr, info.sender, ContractError::SenderIsNotIcs721);

        NFT_CONTRACT.save(deps.storage, &deps.api.addr_validate(&nft_contract)?)?;

        let owner = deps
            .querier
            .query_wasm_smart::<cw721::msg::OwnerOfResponse>(
                nft_contract,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
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
        msg: to_json_binary(&cw721::msg::Cw721ExecuteMsg::<
            DefaultOptionalNftExtensionMsg,
            DefaultOptionalCollectionExtensionMsg,
            Empty,
        >::SendNft {
            contract: ics721,
            token_id,
            msg: to_json_binary(&IbcOutgoingMsg {
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
            data: to_json_binary(&data)?,
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
        QueryMsg::AckMode {} => to_json_binary(&ACK_MODE.load(deps.storage)?),
        QueryMsg::LastAck {} => to_json_binary(&LAST_ACK.load(deps.storage)?),
        QueryMsg::GetReceivedCallback {} => to_json_binary(&RECEIVED_CALLBACK.load(deps.storage)?),
        QueryMsg::GetNftContract {} => to_json_binary(&NFT_CONTRACT.load(deps.storage)?),
        QueryMsg::GetSentCallback {} => to_json_binary(&SENT_CALLBACK.load(deps.storage)?),
    }
}
