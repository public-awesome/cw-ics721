use cosmwasm_std::{
    entry_point, from_binary, to_binary, Deps, DepsMut, Empty, Env, IbcBasicResponse,
    IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcPacket, IbcPacketAckMsg,
    IbcPacketReceiveMsg, IbcPacketTimeoutMsg, IbcReceiveResponse, Reply, Response, StdResult,
    SubMsg, SubMsgResult, WasmMsg,
};
use cw_utils::parse_reply_instantiate_data;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    error::Never,
    helpers::{
        BATCH_TRANSFER_FROM_CHANNEL_REPLY_ID, BURN_ESCROW_TOKENS_REPLY_ID, BURN_SUB_MSG_REPLY_ID,
        FAILURE_RESPONSE_FAILURE_REPLY_ID, INSTANTIATE_AND_MINT_CW721_REPLY_ID,
        INSTANTIATE_CW721_REPLY_ID, INSTANTIATE_ESCROW_REPLY_ID, MINT_SUB_MSG_REPLY_ID,
        TRANSFER_SUB_MSG_REPLY_ID,
    },
    ibc_helpers::{
        ack_fail, ack_success, get_endpoint_prefix, try_get_ack_error, try_pop_source_prefix,
        validate_order_and_version,
    },
    msg::ExecuteMsg,
    state::{CHANNELS, CLASS_ID_TO_NFT_CONTRACT, ESCROW_CODE_ID, NFT_CONTRACT_TO_CLASS_ID},
    ContractError,
};

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NonFungibleTokenPacketData {
    /// Uniquely identifies the collection which the tokens being
    /// transfered belong to on the sending chain.
    pub class_id: String,
    /// URL that points to metadata about the collection. This is not
    /// validated.
    pub class_uri: Option<String>,
    /// Uniquely identifies the tokens in the NFT collection being
    /// transfered.
    pub token_ids: Vec<String>,
    /// URL that points to metadata for each token being
    /// transfered. `tokenUris[N]` should hold the metadata for
    /// `tokenIds[N]` and both lists should have the same length.
    pub token_uris: Vec<String>,
    /// The address sending the tokens on the sending chain.
    pub sender: String,
    /// The address that should receive the tokens on the receiving
    /// chain.
    pub receiver: String,
}

pub const IBC_VERSION: &str = "ics721-1";
const ACK_ERROR_FALLBACK: &str =
    "an unexpected error occurred - error text is hidden because it would serialize as ACK success";

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_channel_open(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelOpenMsg,
) -> Result<(), ContractError> {
    validate_order_and_version(msg.channel(), msg.counterparty_version())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_channel_connect(
    deps: DepsMut,
    env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    validate_order_and_version(msg.channel(), msg.counterparty_version())?;

    let message = ics721_escrow::msg::InstantiateMsg {
        admin_address: env.contract.address.into_string(),
        channel_id: msg.channel().endpoint.channel_id.clone(),
    };
    let message = WasmMsg::Instantiate {
        admin: None,
        code_id: ESCROW_CODE_ID.load(deps.storage)?,
        msg: to_binary(&message)?,
        funds: vec![],
        label: format!(
            "channel ({}) ICS721 escrow",
            msg.channel().endpoint.channel_id
        ),
    };
    let message = SubMsg::<Empty>::reply_always(message, INSTANTIATE_ESCROW_REPLY_ID);

    Ok(IbcBasicResponse::new()
        .add_attribute("method", "ibc_channel_connect")
        .add_attribute("channel", &msg.channel().endpoint.channel_id)
        .add_attribute("port", &msg.channel().endpoint.port_id)
        .add_submessage(message))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_channel_close(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelCloseMsg,
) -> Result<IbcBasicResponse, ContractError> {
    match msg {
        IbcChannelCloseMsg::CloseInit { channel: _ } => Err(ContractError::CantCloseChannel {}),
        IbcChannelCloseMsg::CloseConfirm { channel: _ } => {
            // TODO: Is this actually the correct logic? If we do hit
            // this, IBC is telling us "the channel has been closed
            // despite your objection". Will IBC ever tell us this?
            // Should we release NFTs / remove the channel from
            // CHANNELS if this happens?
            unreachable!("channel can not be closed")
        }
        _ => unreachable!("channel can not be closed"),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_packet_receive(
    deps: DepsMut,
    env: Env,
    msg: IbcPacketReceiveMsg,
) -> Result<IbcReceiveResponse, Never> {
    // Regardless of if our processing of this packet works we need to
    // commit an ACK to the chain. As such, we wrap all handling logic
    // in a seprate function and on error write out an error ack.
    match do_ibc_packet_receive(deps.as_ref(), env, msg.packet) {
        Ok(response) => Ok(response),
        Err(error) => Ok(IbcReceiveResponse::new()
            .add_attribute("method", "ibc_packet_receive")
            .add_attribute("error", error.to_string())
            .set_ack(ack_fail(&error.to_string()).unwrap())),
    }
}

fn do_ibc_packet_receive(
    _deps: Deps,
    env: Env,
    packet: IbcPacket,
) -> Result<IbcReceiveResponse, ContractError> {
    let data: NonFungibleTokenPacketData = from_binary(&packet.data)?;
    data.validate()?;

    // Check if this token is returning to this chain. If it is, we
    // pop the path from the classID.
    if let Some(class_id) = try_pop_source_prefix(&packet.src, &data.class_id) {
        // The token has previously left this chain to go to the other
        // chain and is in the escrow. Unescrow the token and give it
        // to the receiver.
        //
        // For each tokenID:
        //   1. Get the escrow address for this destination port and
        //      channel.
        //   2. Get the cw721 address for this classID.
        //   3. Transfer the tokenID from escrow to receiver.
        //
        // See `execute_receive_nft` for a description of why this works.

        // The escrow protects us from overwithdrawal by an evil chain
        // because the cw721s underlying all this will start failing
        // the transactions as the escrow for the channel will run out
        // of NFTs. If we held all these in the bridge contract a
        // malicious chain could connect to us using the same source
        // prefix as a different chain and drain all of its funds.
        //
        // TL;DR - Don't get rid of the escrows. Make sure to use
        // `packet.dest` in the channel field below.

        // Transfer all of the requested NFTs out of the channel and
        // to the receiver.
        let message = ExecuteMsg::BatchTransferFromChannel {
            channel: packet.dest.channel_id,
            class_id: class_id.to_string(),
            token_ids: data.token_ids,
            receiver: data.receiver,
        };

        let message = WasmMsg::Execute {
            contract_addr: env.contract.address.into_string(),
            msg: to_binary(&message)?,
            funds: vec![],
        };
        let message = SubMsg::reply_always(message, BATCH_TRANSFER_FROM_CHANNEL_REPLY_ID);

        Ok(IbcReceiveResponse::default()
            .add_attribute("method", "ics721_transfer_source")
            .add_submessage(message))
    } else {
        // The token is being sent to this chain from another
        // one. Push to classID and dispatch submessage to create new x
        // cw721 (if needed) and mint for the receiver.
        let local_prefix = get_endpoint_prefix(&packet.dest);
        let local_class_id = format!("{}{}", local_prefix, data.class_id);

        // We can not dispatch multiple submessages and still handle
        // errors and rollbacks correctly [1]. As such, we bundle
        // these steps into one message that is only callable by the
        // contract itself.
        //
        // [1] https://github.com/CosmWasm/cosmwasm/blob/main/IBC.md#acknowledging-errors
        let ibc_ack_check_message = SubMsg::reply_always(
            WasmMsg::Execute {
                contract_addr: env.contract.address.into_string(),
                msg: to_binary(&ExecuteMsg::DoInstantiateAndMint {
                    class_id: local_class_id,
                    class_uri: data.class_uri,
                    token_ids: data.token_ids,
                    token_uris: data.token_uris,
                    // FIXME: ics20 seems to set the receiver field as a
                    // bech32 address. IF we need to do this, need to convert
                    // first.
                    receiver: data.receiver,
                })?,
                funds: vec![],
            },
            INSTANTIATE_AND_MINT_CW721_REPLY_ID,
        );

        // Dispatch submessage. We DO NOT set the ack here as it will
        // be set in the submessage reply handler if all goes well.
        Ok(IbcReceiveResponse::default()
            .add_attribute("method", "ics721_transfer_sink")
            .add_submessage(ibc_ack_check_message))
    }
}

// TODO: document that this will only be called in response to me
// sending a NFT somewhere.
#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_packet_ack(
    deps: DepsMut,
    env: Env,
    ack: IbcPacketAckMsg,
) -> Result<IbcBasicResponse, ContractError> {
    if let Some(error) = try_get_ack_error(&ack.acknowledgement)? {
        handle_packet_fail(deps.as_ref(), ack.original_packet, &error)
    } else {
        let msg: NonFungibleTokenPacketData = from_binary(&ack.original_packet.data)?;

        // We only receive ACKs from our own packets. As such, if we
        // get an ACK it means that we have sent a IBC message. If
        // we're here, it means it has succeded.
        //
        // Now, if we were the sink chain for this NFT and the NFT is
        // returning to its source chain, we need to burn it. For
        // example, if chain B sent us a NFT and then it got sent back
        // to chain B we should not keep that NFT in an escrow. This
        // is because the purpose of the escrows is to do book keeping
        // for _outgoing_ NFTs.
        //
        // We can't do the actual burning here because this method
        // should be infalliable.
        let prefix = get_endpoint_prefix(&ack.original_packet.src);
        let messages = if msg.class_id.starts_with(&prefix) {
            let message = WasmMsg::Execute {
                contract_addr: env.contract.address.into_string(),
                msg: to_binary(&ExecuteMsg::BurnEscrowTokens {
                    channel: ack.original_packet.src.channel_id,
                    class_id: msg.class_id.clone(),
                    token_ids: msg.token_ids.clone(),
                })?,
                funds: vec![],
            };
            let message = SubMsg::reply_always(message, BURN_ESCROW_TOKENS_REPLY_ID);
            vec![message]
        } else {
            vec![]
        };

        // cw-plus' ics20 implementation doesn't add an ack_success
        // here. TODO: should we?
        Ok(IbcBasicResponse::new()
            .add_submessages(messages)
            .add_attribute("method", "acknowledge")
            .add_attribute("sender", msg.sender)
            .add_attribute("receiver", msg.receiver)
            .add_attribute("classId", msg.class_id)
            .add_attribute("token_ids", format!("{:?}", msg.token_ids)))
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn ibc_packet_timeout(
    deps: DepsMut,
    _env: Env,
    msg: IbcPacketTimeoutMsg,
) -> Result<IbcBasicResponse, ContractError> {
    handle_packet_fail(deps.as_ref(), msg.packet, "timeout")
}

fn handle_packet_fail(
    deps: Deps,
    packet: IbcPacket,
    error: &str,
) -> Result<IbcBasicResponse, ContractError> {
    // On fail, return the NFT from the escrow. We'll only ever handle
    // our own packets in this method so we use the packet source
    // which, as a result, is trusted.
    let escrow_addr = CHANNELS.load(deps.storage, packet.src.channel_id.clone())?;

    // If this deserialization fails something truly strange has
    // happened. TODO: do we need to handle this? Otherwise, this
    // method is faliable.
    let message: NonFungibleTokenPacketData = from_binary(&packet.data)?;
    let nft_address = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, message.class_id.clone())?;

    let return_nfts = message
        .token_ids
        .iter() // Can't into_iter() here because we use a reference in the closure.
        .map(|token_id| -> StdResult<SubMsg<Empty>> {
            let wasm = WasmMsg::Execute {
                contract_addr: escrow_addr.to_string(),
                msg: to_binary(&ics721_escrow::msg::ExecuteMsg::Withdraw {
                    nft_address: nft_address.to_string(),
                    token_id: token_id.clone(),
                    receiver: message.sender.clone(),
                })?, // FIXME: how do we handle a failure here?
                funds: vec![],
            };
            // cw-plus' ics20 implementation fail ACKs the fail, so we
            // do to. More likely than not, more than one of these
            // will fail if any fail and there are more than one. In
            // that case, the ack_fail will still get written as it'll
            // just repeatedly override itself.
            Ok(SubMsg::reply_on_error(
                wasm,
                FAILURE_RESPONSE_FAILURE_REPLY_ID,
            ))
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(IbcBasicResponse::new()
        .add_submessages(return_nfts)
        .add_attribute("method", "handle_packet_fail")
        .add_attribute("token_ids", format!("{:?}", message.token_ids))
        .add_attribute("class_id", message.class_id)
        .add_attribute("escrow", escrow_addr)
        .add_attribute("channel_id", packet.src.channel_id)
        .add_attribute("address_refunded", message.sender)
        .add_attribute("error", error))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        INSTANTIATE_CW721_REPLY_ID => {
            // Don't need to add an ack or check for an error here as this
            // is only replies on success. This is OK because it is only
            // ever used in `DoInstantiateAndMint` which itself is always
            // a submessage of `ibc_packet_receive` which is caught and
            // handled correctly by the reply handler for
            // `INSTANTIATE_AND_MINT_CW721_REPLY_ID`.

            let res = parse_reply_instantiate_data(reply)?;
            let cw721_addr = deps.api.addr_validate(&res.contract_address)?;

            // We need to map this address back to a class
            // ID. Fourtunately, we set the name of the new NFT
            // contract to the class ID.
            let cw721::ContractInfoResponse { name: class_id, .. } = deps
                .querier
                .query_wasm_smart(cw721_addr.clone(), &cw721::Cw721QueryMsg::ContractInfo {})?;

            // Save classId <-> contract mappings.
            CLASS_ID_TO_NFT_CONTRACT.save(deps.storage, class_id.clone(), &cw721_addr)?;
            NFT_CONTRACT_TO_CLASS_ID.save(deps.storage, cw721_addr.clone(), &class_id)?;

            Ok(Response::default()
                .add_attribute("method", "instantiate_cw721_reply")
                .add_attribute("class_id", class_id)
                .add_attribute("cw721_addr", cw721_addr))
        }
        INSTANTIATE_ESCROW_REPLY_ID => {
            if let SubMsgResult::Err(err) = reply.result {
                return Ok(Response::new().set_data(
                    ack_fail(&err).unwrap_or_else(|_e| ack_fail(ACK_ERROR_FALLBACK).unwrap()),
                ));
            }

            let res = parse_reply_instantiate_data(reply)?;
            let escrow_addr = deps.api.addr_validate(&res.contract_address)?;

            let channel_id: String = deps.querier.query_wasm_smart(
                escrow_addr.clone(),
                &ics721_escrow::msg::QueryMsg::ChannelId {},
            )?;

            CHANNELS.save(deps.storage, channel_id.clone(), &escrow_addr)?;

            // This reply gets called from `ibc_channel_connect` so we
            // need to add an ack.
            Ok(Response::default()
                .add_attribute("method", "instantiate_escrow_reply")
                .add_attribute("escrow_addr", escrow_addr)
                .add_attribute("channel_id", channel_id)
                .set_data(ack_success()))
        }
        // These messages don't need to do any state changes in the
        // reply - just need to commit an ack.
        MINT_SUB_MSG_REPLY_ID
        | TRANSFER_SUB_MSG_REPLY_ID
        | BURN_SUB_MSG_REPLY_ID
        | INSTANTIATE_AND_MINT_CW721_REPLY_ID
        | BATCH_TRANSFER_FROM_CHANNEL_REPLY_ID
        | BURN_ESCROW_TOKENS_REPLY_ID
        | FAILURE_RESPONSE_FAILURE_REPLY_ID => {
            match reply.result {
                // On success, set a successful ack. Nothing else to do.
                SubMsgResult::Ok(_) => Ok(Response::new().set_data(ack_success())),
                // On error we need to use set_data to override the data field
                // from our caller, the IBC packet recv, and acknowledge our
                // failure.  As per:
                // https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
                SubMsgResult::Err(err) => Ok(Response::new().set_data(
                    ack_fail(&err).unwrap_or_else(|_e| ack_fail(ACK_ERROR_FALLBACK).unwrap()),
                )),
            }
        }
        _ => Err(ContractError::UnrecognisedReplyId {}),
    }
}
