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

    #[error("channel may not be closed")]
    CantCloseChannel {},

    #[error("unrecognised reply ID")]
    UnrecognisedReplyId {},

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error("tokenId list has different length than tokenUri list")]
    TokenInfoLenMissmatch {},
}

/// Enum that can never be constructed. Used as an error type where we
/// can not error.
#[derive(Error, Debug)]
pub enum Never {}
