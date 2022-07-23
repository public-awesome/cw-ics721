use cosmwasm_std::{
    from_binary, to_binary, Binary, IbcAcknowledgement, IbcChannel, IbcEndpoint, IbcOrder,
    StdError, StdResult,
};

use crate::{
    ibc::{NonFungibleTokenPacketData, IBC_VERSION},
    ContractError,
};

/// Tries to remove the source prefix from a given class_id. If the
/// class_id does not begin with the given prefix, returns
/// `None`. Otherwise, returns `Some(unprefixed)`.
pub(crate) fn try_pop_source_prefix<'a>(
    source: &IbcEndpoint,
    class_id: &'a str,
) -> Option<&'a str> {
    let source_prefix = get_endpoint_prefix(source);
    // This must not panic in the face of non-ascii, or empty
    // strings. We can not trust classID as it comes from an external
    // IBC connection.
    class_id.strip_prefix(&source_prefix)
}

/// Gets the classID prefix for a given IBC endpoint.
pub(crate) fn get_endpoint_prefix(source: &IbcEndpoint) -> String {
    format!("{}/{}/", source.port_id, source.channel_id)
}

/// Success ACK. 0x01 base64 encoded. By 0x01 base64 encoded, this
/// literally means it is the base64 encoding of the number 1. You can
/// test this by pasting this into a base64 decoder and, if it's for
/// text, it'll output ascii character "START OF HEADING".
pub fn ack_success() -> Binary {
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
pub fn ack_fail(message: &str) -> StdResult<Binary> {
    if message == "AQ==" {
        Err(StdError::serialize_err(
            message,
            "ACK fail would have the same encoding as ACK success.",
        ))
    } else {
        to_binary(message)
    }
}

pub(crate) fn try_get_ack_error(ack: &IbcAcknowledgement) -> StdResult<Option<String>> {
    let msg: String = from_binary(&ack.data)?;
    Ok(if msg != "AQ==" { Some(msg) } else { None })
}

/// Validates order and version information for ics721. We expect
/// ics721-1 as the version and an unordered channel.
pub(crate) fn validate_order_and_version(
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

impl NonFungibleTokenPacketData {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.tokenIds.len() != self.tokenUris.len() {
            return Err(ContractError::TokenInfoLenMissmatch {});
        }

        // TODO: Should we check the tokenIds field for duplicates?
        // O(log(N)). A well behaved cw721 implementation will catch
        // this downstream if we try and mint / trasnfer the same
        // token twice.

        Ok(())
    }
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
