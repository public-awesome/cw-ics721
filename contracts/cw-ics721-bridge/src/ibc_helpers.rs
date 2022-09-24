use cosmwasm_std::{
    from_binary, to_binary, Binary, IbcAcknowledgement, IbcChannel, IbcEndpoint, IbcOrder,
};

use serde::{Deserialize, Serialize};

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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ics20Ack {
    Result(Binary),
    Error(String),
}

pub fn ack_success() -> Binary {
    let res = Ics20Ack::Result(b"1".into());
    to_binary(&res).unwrap()
}

pub fn ack_fail(err: String) -> Binary {
    let res = Ics20Ack::Error(err);
    to_binary(&res).unwrap()
}

/// Tries to get the error from an ACK. If an error exists, returns
/// Some(error_message). Otherwise, returns `None`.
///
/// NOTE(ekez): there is a special case here where the contents of the
/// ACK we receive are set by the SDK, and not by our counterparty
/// contract. I do not know all cases this will occur, but I do know
/// it happens if a field on the packet data is set to an empty
/// string. That being the case, the SDK will return an error in the
/// form:
///
/// ```json
/// {"error":"Empty attribute value. Key: class_id: invalid event"}
/// ```
///
/// Should this method encounter such an error, it will return a
/// base64 encoded version of the error (as this is what it
/// receives). For example, the above error is returned as:
///
/// ```json
/// "eyJlcnJvciI6IkVtcHR5IGF0dHJpYnV0ZSB2YWx1ZS4gS2V5OiBjbGFzc19pZDogaW52YWxpZCBldmVudCJ9"
/// ```
pub fn try_get_ack_error(ack: &IbcAcknowledgement) -> Option<String> {
    let ack: Ics20Ack =
	// What we can not parse is an ACK fail.
        from_binary(&ack.data).unwrap_or_else(|_| Ics20Ack::Error(ack.data.to_base64()));
    match ack {
        Ics20Ack::Error(e) => Some(e),
        _ => None,
    }
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
        if self.token_ids.len() != self.token_uris.len() {
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
