use cosmwasm_schema::cw_serde;
use cosmwasm_std::Binary;

use crate::{
    error::Ics721Error,
    token_types::{ClassId, TokenId},
};

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
            return Err(Ics721Error::EmptyOptional);
        }
    };
}

impl NonFungibleTokenPacketData {
    pub fn validate(&self) -> Result<(), Ics721Error> {
        if self.class_id.is_empty() {
            return Err(Ics721Error::EmptyClassId);
        }

        non_empty_optional!(self.class_uri.as_ref());
        non_empty_optional!(self.class_data.as_ref());

        let token_count = self.token_ids.len();
        if token_count == 0 {
            return Err(Ics721Error::NoTokens);
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
            return Err(Ics721Error::TokenInfoLenMissmatch);
        }

        // This contract assumes that the backing cw721 is functional,
        // so no need to check tokenIds for duplicates as the cw721
        // will prevent minting of duplicates.

        Ok(())
    }
}
