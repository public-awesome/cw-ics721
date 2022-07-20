use cosmwasm_std::{
    entry_point, from_binary, to_binary, Binary, Deps, DepsMut, Empty, Env, IbcBasicResponse,
    IbcChannel, IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcEndpoint, IbcOrder,
    IbcPacket, IbcPacketReceiveMsg, IbcReceiveResponse, Reply, Response, StdError, StdResult,
    SubMsg, SubMsgResult, WasmMsg,
};
use cw_utils::parse_reply_instantiate_data;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::error::Never;
use crate::helpers::{
    BURN_SUB_MSG_REPLY_ID, INSTANTIATE_AND_MINT_CW721_REPLY_ID, INSTANTIATE_CW721_REPLY_ID,
    MINT_SUB_MSG_REPLY_ID, TRANSFER_SUB_MSG_REPLY_ID,
};
use crate::msg::ExecuteMsg;
use crate::state::{CLASS_ID_TO_NFT_CONTRACT, NFT_CONTRACT_TO_CLASS_ID};
use crate::{state::CHANNELS, ContractError};

#[derive(Serialize, Deserialize, JsonSchema)]
#[allow(non_snake_case)]
pub struct NonFungibleTokenPacketData {
    /// Uniquely identifies the collection which the tokens being
    /// transfered belong to on the sending chain.
    pub classId: String,
    /// URL that points to metadata about the collection. This is not
    /// validated.
    pub classUrl: Option<String>,
    /// Uniquely identifies the tokens in the NFT collection being
    /// transfered.
    pub tokenIds: Vec<String>,
    /// URL that points to metadata for each token being
    /// transfered. `tokenUris[N]` should hold the metadata for
    /// `tokenIds[N]` and both lists should have the same length.
    pub tokenUris: Vec<String>,
    /// The address sending the tokens on the sending chain.
    pub sender: String,
    /// The address that should receive the tokens on the receiving
    /// chain.
    pub receiver: String,
}

pub const IBC_VERSION: &str = "ics721-1";

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
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<IbcBasicResponse, ContractError> {
    validate_order_and_version(msg.channel(), msg.counterparty_version())?;

    CHANNELS.save(
        deps.storage,
        msg.channel().endpoint.channel_id.clone(),
        &Empty {},
    )?;

    Ok(IbcBasicResponse::new().add_attribute("method", "ibc_channel_connect"))
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

    // Check if this token is returning to this chain. If it is, we
    // pop the path from the classID.
    if let Some(_unprefixed) = try_pop_source_prefix(&packet.src, &data.classId) {
        // The token has previously left this chain to go to the other
        // chain and is in the escrow. Unescrow the token and give it
        // to the receiver.
        //
        // For each tokenID:
        //   1. Get the escrow address for this destination port and
        //      channel.
        //   2. Get the cw721 address for this classID.
        //   3. Transfer the tokenID from escrow to receiver.
        todo!()
    } else {
        // The token is being sent to this chain from another
        // one. Push to classID and dispatch submessage to create new
        // cw721 (if needed) and mint for the receiver.
        let local_prefix = get_endpoint_prefix(&packet.dest);
        let local_class_id = format!("{}{}", local_prefix, data.classId);

        // We can not dispatch multiple submessages and still handle
        // errors and rollbacks correctly [1]. As such, we bundle
        // these steps into one message that is only callable by the
        // contract itself.
        //
        // [1] https://github.com/CosmWasm/cosmwasm/blob/main/IBC.md#acknowledging-errors
        let message = ExecuteMsg::DoInstantiateAndMint {
            class_id: local_class_id,
            token_ids: data.tokenIds,
            token_uris: data.tokenUris,
            // FIXME: ics20 seems to set the receiver field as a
            // bech32 address. IF we need to do this, need to convert
            // first.
            receiver: data.receiver,
        };
        let message = WasmMsg::Execute {
            contract_addr: env.contract.address.into_string(),
            msg: to_binary(&message)?,
            funds: vec![],
        };
        let message = SubMsg::reply_always(message, INSTANTIATE_AND_MINT_CW721_REPLY_ID);

        // Dispatch submessage. We DO NOT set the ack here as it will
        // be set in the submessage reply handler if all goes well.
        Ok(IbcReceiveResponse::default()
            .add_attribute("method", "ics_transfer_in")
            .add_submessage(message))
    }
}

/// Tries to remove the source prefix from a given class_id. If the
/// class_id does not begin with the given prefix, returns
/// `None`. Otherwise, returns `Some(unprefixed)`.
fn try_pop_source_prefix<'a>(source: &IbcEndpoint, class_id: &'a str) -> Option<&'a str> {
    let source_prefix = get_endpoint_prefix(source);
    // This must not panic in the face of non-ascii, or empty
    // strings. We can not trust classID as it comes from an external
    // IBC connection.
    class_id.strip_prefix(&source_prefix)
}

/// Gets the classID prefix for a given IBC endpoint.
fn get_endpoint_prefix(source: &IbcEndpoint) -> String {
    format!("{}/{}/", source.port_id, source.channel_id)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn reply(deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        INSTANTIATE_CW721_REPLY_ID => {
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
        MINT_SUB_MSG_REPLY_ID
        | TRANSFER_SUB_MSG_REPLY_ID
        | BURN_SUB_MSG_REPLY_ID
        | INSTANTIATE_AND_MINT_CW721_REPLY_ID => {
            match reply.result {
	    // On success, set a successful ack. Nothing else to do.
            SubMsgResult::Ok(_) => Ok(Response::new().set_data(ack_success())),
            // On error we need to use set_data to override the data field
            // from our caller, the IBC packet recv, and acknowledge our
            // failure.  As per:
            // https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
            SubMsgResult::Err(err) => Ok(Response::new().set_data(
		ack_fail(&err).unwrap_or_else(|_e| ack_fail("an unexpected error occurred - error text is hidden because it would serialize as ACK success").unwrap()),
            )),
	}
        }
        _ => Err(ContractError::UnrecognisedReplyId {}),
    }
}

/// Success ACK. 0x01 base64 encoded. By 0x01 base64 encoded, this
/// literally means it is the base64 encoding of the number 1. You can
/// test this by pasting this into a base64 decoder and, if it's for
/// text, it'll output ascii character "START OF HEADING".
fn ack_success() -> Binary {
    // From the spec:
    //
    // > "Note that ... NonFungibleTokenPacketAcknowledgement must be
    // > JSON-encoded (not Protobuf encoded) when serialized into packet
    // > data."
    //
    // As such we encode '"AQ=="' as in JSON strings are surrounded by
    // quotation marks as 'AQ==' is the base64 encoding of the number
    // 1. The binary (ASCII code point list) version of this is below
    // as we are dealing with a constant value.
    Binary::from([34, 65, 81, 61, 61, 34])
}

/// Fail ACK. Contains some arbitrary message. This message can not be
/// 'AQ==' otherwise it will be parsed as a success message.
fn ack_fail(message: &str) -> StdResult<Binary> {
    if message == "AQ==" {
        Err(StdError::serialize_err(
            message,
            "ACK fail would have the same encoding as ACK success.",
        ))
    } else {
        to_binary(message)
    }
}

/// Validates order and version information for ics721. We expect
/// ics721-1 as the version and an unordered channel.
fn validate_order_and_version(
    channel: &IbcChannel,
    counterparty_version: Option<&str>,
) -> Result<(), ContractError> {
    // We expect an unordered channel here. Ordered channels have the
    // property that if a message is lost the entire channel will stop
    // working until you start it again.
    if channel.order != IbcOrder::Unordered {
        return Err(ContractError::OrderedChannel {});
    }

    if channel.version != IBC_VERSION {
        return Err(ContractError::InvalidVersion {
            actual: channel.version.to_string(),
            expected: IBC_VERSION.to_string(),
        });
    }

    // Make sure that we're talking with a counterparty who speaks the
    // same "protocol" as us.
    //
    // For a connection between chain A and chain B being established
    // by chain A, chain B knows counterparty information during
    // `OpenTry` and chain A knows counterparty information during
    // `OpenAck`. We verify it when we have it but when we don't it's
    // alright.
    if let Some(counterparty_version) = counterparty_version {
        if counterparty_version != IBC_VERSION {
            return Err(ContractError::InvalidVersion {
                actual: counterparty_version.to_string(),
                expected: IBC_VERSION.to_string(),
            });
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ack_success_encoding() {
        // Our implementation doesn't use to_binary and instead just
        // builds the byte array manually as it is constant. Make sure
        // that we're always in sync wih the non-manual version.
        assert_eq!(ack_success(), to_binary("AQ==").unwrap())
    }

    #[test]
    fn test_pop_source_simple() {
        assert_eq!(
            try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                "wasm.address1/channel-10/address2"
            ),
            Some("address2")
        )
    }

    #[test]
    fn test_pop_source_adversarial() {
        // Empty string.
        assert_eq!(
            try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                ""
            ),
            None
        );

        // Non-ASCII
        assert_eq!(
            try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                "☯️☯️"
            ),
            None
        );

        // Invalid classID from prohibited '/' characters.
        assert_eq!(
            try_pop_source_prefix(
                &IbcEndpoint {
                    port_id: "wasm.address1".to_string(),
                    channel_id: "channel-10".to_string(),
                },
                "wasm.addre//1/channel-10/addre//2"
            ),
            None
        );
    }
}
