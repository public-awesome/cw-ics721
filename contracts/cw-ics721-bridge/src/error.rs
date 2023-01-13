use cosmwasm_std::StdError;
use cw_pause_once::PauseError;
use cw_utils::ParseReplyError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    Pause(#[from] PauseError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("only unordered channels are supported")]
    OrderedChannel {},

    #[error("invalid IBC channel version - got ({actual}), expected ({expected})")]
    InvalidVersion { actual: String, expected: String },

    #[error("ICS 721 channels may not be closed")]
    CantCloseChannel {},

    #[error("unrecognised class ID")]
    UnrecognisedClassId {},

    #[error("class ID already exists")]
    ClassIdAlreadyExists {},

    #[error("empty class ID")]
    EmptyClassId {},

    #[error("must transfer at least one token")]
    NoTokens {},

    #[error("optional fields may not be empty if provided")]
    EmptyOptional {},

    #[error("unrecognised reply ID")]
    UnrecognisedReplyId {},

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error("must provide same number of token IDs and URIs")]
    ImbalancedTokenInfo {},

    #[error("unexpected uri for classID {class_id} - got ({actual:?}), expected ({expected:?})")]
    ClassUriClash {
        class_id: String,
        expected: Option<String>,
        actual: Option<String>,
    },

    #[error("tokenIds, tokenUris, and tokenData must have the same length")]
    TokenInfoLenMissmatch {},
}

/// Enum that can never be constructed. Used as an error type where we
/// can not error.
#[derive(Error, Debug)]
pub enum Never {}
