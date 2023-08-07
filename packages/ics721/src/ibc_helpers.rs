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

/// The ICS721 spec is very vague about how ACKs are suposed to be
/// encoded. To be honest, I don't think this method is correct at all
/// if we were to follow the wording of the spec.
///
/// The intent of the spec though is to have the same ACK format as
/// ICS20 which endodes its ACKs like this. This is compatible with
/// the SDK ACK protobuf defined here:
/// <https://github.com/cosmos/cosmos-sdk/blob/v0.42.0/proto/ibc/core/channel/v1/channel.proto#L141-L147>
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ics721Ack {
    Result(Binary),
    Error(String),
}

pub fn ack_success() -> Binary {
    let res = Ics721Ack::Result(b"1".into());
    to_binary(&res).unwrap()
}

pub fn ack_fail(err: String) -> Binary {
    let res = Ics721Ack::Error(err);
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
    let ack: Ics721Ack =
	// What we can not parse is an ACK fail.
        from_binary(&ack.data).unwrap_or_else(|_| Ics721Ack::Error(ack.data.to_base64()));
    match ack {
        Ics721Ack::Error(e) => Some(e),
        _ => None,
    }
}

/// Validates order and version information for ics721. We expect
/// ics721-1 as the version and an unordered channel.
pub(crate) fn validate_order_and_version(
    channel: &IbcChannel,
    counterparty_version: Option<&str>,
) -> Result<(), ContractError> {
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

macro_rules! non_empty_optional {
    ($e:expr) => {
        if $e.map_or(false, |data| data.is_empty()) {
            return Err(ContractError::EmptyOptional {});
        }
    };
}

impl NonFungibleTokenPacketData {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.class_id.is_empty() {
            return Err(ContractError::EmptyClassId {});
        }

        non_empty_optional!(self.class_uri.as_ref());
        non_empty_optional!(self.class_data.as_ref());

        let token_count = self.token_ids.len();
        if token_count == 0 {
            return Err(ContractError::NoTokens {});
        }

        // Non-empty optionality of tokenData an tokenUris implicitly
        // checked here.
        if self
            .token_data
            .as_ref()
            .map_or(false, |data| data.len() != token_count)
            || self
                .token_uris
                .as_ref()
                .map_or(false, |data| data.len() != token_count)
        {
            return Err(ContractError::TokenInfoLenMissmatch {});
        }

        // This contract assumes that the backing cw721 is functional,
        // so no need to check tokenIds for duplicates as the cw721
        // will prevent minting of duplicates.

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_types::{ClassId, TokenId};

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

    #[test]
    fn test_packet_validation() {
        let default_token = NonFungibleTokenPacketData {
            class_id: ClassId::new("id"),
            class_uri: None,
            class_data: None,
            token_ids: vec![TokenId::new("1")],
            token_uris: None,
            token_data: None,
            sender: "violet".to_string(),
            receiver: "blue".to_string(),
            memo: None,
        };

        let empty_class_id = NonFungibleTokenPacketData {
            class_id: ClassId::new(""),
            ..default_token.clone()
        };
        let err = empty_class_id.validate().unwrap_err();
        assert_eq!(err, ContractError::EmptyClassId {});

        let empty_class_uri = NonFungibleTokenPacketData {
            class_uri: Some("".to_string()),
            ..default_token.clone()
        };
        let err = empty_class_uri.validate().unwrap_err();
        assert_eq!(err, ContractError::EmptyOptional {});

        let empty_class_data = NonFungibleTokenPacketData {
            class_data: Some(Binary::default()),
            ..default_token.clone()
        };
        let err = empty_class_data.validate().unwrap_err();
        assert_eq!(err, ContractError::EmptyOptional {});

        let no_tokens = NonFungibleTokenPacketData {
            token_ids: vec![],
            ..default_token.clone()
        };
        let err = no_tokens.validate().unwrap_err();
        assert_eq!(err, ContractError::NoTokens {});

        let uri_imbalance_empty = NonFungibleTokenPacketData {
            token_uris: Some(vec![]),
            ..default_token.clone()
        };
        let err = uri_imbalance_empty.validate().unwrap_err();
        assert_eq!(err, ContractError::TokenInfoLenMissmatch {});

        let uri_imbalance = NonFungibleTokenPacketData {
            token_uris: Some(vec!["a".to_string(), "b".to_string()]),
            ..default_token.clone()
        };
        let err = uri_imbalance.validate().unwrap_err();
        assert_eq!(err, ContractError::TokenInfoLenMissmatch {});

        let data_imbalance_empty = NonFungibleTokenPacketData {
            token_data: Some(vec![]),
            ..default_token.clone()
        };
        let err = data_imbalance_empty.validate().unwrap_err();
        assert_eq!(err, ContractError::TokenInfoLenMissmatch {});

        let data_imbalance = NonFungibleTokenPacketData {
            token_data: Some(vec![Binary::default(), Binary::default()]),
            ..default_token
        };
        let err = data_imbalance.validate().unwrap_err();
        assert_eq!(err, ContractError::TokenInfoLenMissmatch {});
    }
}
