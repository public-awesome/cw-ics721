use cosmwasm_schema::cw_serde;
use cosmwasm_std::Binary;

use crate::{token_types::{ClassId, TokenId}, error::ValidationError};

#[cw_serde]
#[serde(rename_all = "camelCase")]
pub struct NonFungibleTokenPacketData {
    /// Uniquely identifies the collection which the tokens being
    /// transfered belong to on the sending chain. Must be non-empty.
    pub class_id: ClassId,
    /// Optional URL that points to metadata about the
    /// collection. Must be non-empty if provided.
    pub class_uri: Option<String>,
    /// Optional base64 encoded field which contains on-chain metadata
    /// about the NFT class. Must be non-empty if provided.
    pub class_data: Option<Binary>,
    /// Uniquely identifies the tokens in the NFT collection being
    /// transfered. This MUST be non-empty.
    pub token_ids: Vec<TokenId>,
    /// Optional URL that points to metadata for each token being
    /// transfered. `tokenUris[N]` should hold the metadata for
    /// `tokenIds[N]` and both lists should have the same if
    /// provided. Must be non-empty if provided.
    pub token_uris: Option<Vec<String>>,
    /// Optional base64 encoded metadata for the tokens being
    /// transfered. `tokenData[N]` should hold metadata for
    /// `tokenIds[N]` and both lists should have the same length if
    /// provided. Must be non-empty if provided.
    pub token_data: Option<Vec<Binary>>,

    /// The address sending the tokens on the sending chain.
    pub sender: String,
    /// The address that should receive the tokens on the receiving
    /// chain.
    pub receiver: String,
    /// Memo to add custom string to the msg
    pub memo: Option<String>,
}

macro_rules! non_empty_optional {
    ($e:expr) => {
        if $e.map_or(false, |data| data.is_empty()) {
            return Err(ValidationError::EmptyOptional {});
        }
    };
}

impl NonFungibleTokenPacketData {
    pub fn validate(&self) -> Result<(), ValidationError> {
        if self.class_id.is_empty() {
            return Err(ValidationError::EmptyClassId {});
        }

        non_empty_optional!(self.class_uri.as_ref());
        non_empty_optional!(self.class_data.as_ref());

        let token_count = self.token_ids.len();
        if token_count == 0 {
            return Err(ValidationError::NoTokens {});
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
            return Err(ValidationError::TokenInfoLenMissmatch {});
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
        assert_eq!(err, ValidationError::EmptyClassId {});

        let empty_class_uri = NonFungibleTokenPacketData {
            class_uri: Some("".to_string()),
            ..default_token.clone()
        };
        let err = empty_class_uri.validate().unwrap_err();
        assert_eq!(err, ValidationError::EmptyOptional {});

        let empty_class_data = NonFungibleTokenPacketData {
            class_data: Some(Binary::default()),
            ..default_token.clone()
        };
        let err = empty_class_data.validate().unwrap_err();
        assert_eq!(err, ValidationError::EmptyOptional {});

        let no_tokens = NonFungibleTokenPacketData {
            token_ids: vec![],
            ..default_token.clone()
        };
        let err = no_tokens.validate().unwrap_err();
        assert_eq!(err, ValidationError::NoTokens {});

        let uri_imbalance_empty = NonFungibleTokenPacketData {
            token_uris: Some(vec![]),
            ..default_token.clone()
        };
        let err = uri_imbalance_empty.validate().unwrap_err();
        assert_eq!(err, ValidationError::TokenInfoLenMissmatch {});

        let uri_imbalance = NonFungibleTokenPacketData {
            token_uris: Some(vec!["a".to_string(), "b".to_string()]),
            ..default_token.clone()
        };
        let err = uri_imbalance.validate().unwrap_err();
        assert_eq!(err, ValidationError::TokenInfoLenMissmatch {});

        let data_imbalance_empty = NonFungibleTokenPacketData {
            token_data: Some(vec![]),
            ..default_token.clone()
        };
        let err = data_imbalance_empty.validate().unwrap_err();
        assert_eq!(err, ValidationError::TokenInfoLenMissmatch {});

        let data_imbalance = NonFungibleTokenPacketData {
            token_data: Some(vec![Binary::default(), Binary::default()]),
            ..default_token
        };
        let err = data_imbalance.validate().unwrap_err();
        assert_eq!(err, ValidationError::TokenInfoLenMissmatch {});
    }
}
