use cosmwasm_std::{
    entry_point, to_binary, Binary, DepsMut, Empty, Env, IbcBasicResponse, IbcChannel,
    IbcChannelCloseMsg, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcOrder, Reply, Response,
    StdError, StdResult, SubMsgResult,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::helpers::{BURN_SUB_MSG_REPLY_ID, MINT_SUB_MSG_REPLY_ID, TRANSFER_SUB_MSG_REPLY_ID};
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
pub fn reply(_deps: DepsMut, _env: Env, reply: Reply) -> Result<Response, ContractError> {
    match reply.id {
        MINT_SUB_MSG_REPLY_ID => Ok(()),
        TRANSFER_SUB_MSG_REPLY_ID => Ok(()),
        BURN_SUB_MSG_REPLY_ID => Ok(()),
        _ => Err(ContractError::UnrecognisedReplyId {}),
    }?;

    match reply.result {
        // On success we do nothing as all is ok!
        SubMsgResult::Ok(_) => Ok(Response::new()),
        // On error we need to use set_data to override the data field from our
        // caller, the IBC packet recv, and acknowledge our failure.
        // As per: https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#handling-the-reply
        SubMsgResult::Err(err) => Ok(Response::new().set_data(
            ack_fail(&err).unwrap_or_else(|_e| ack_fail("An unexpected error occurred - error text is hidden because it would serialize as ACK success.").unwrap()),
        )),
    }
}

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
            unreachable!("Channel can not be closed.")
        }
        _ => unreachable!("Channel can not be closed."),
    }
}

/// Success ACK. 0x01 base64 encoded.
#[allow(dead_code)]
fn ack_success() -> StdResult<Binary> {
    to_binary("AQ==")
}

/// Fail ACK. Contains some arbitrary message. This message can not be
/// 'AQ==' otherwise it will be parsed as a success message.
#[allow(dead_code)]
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
