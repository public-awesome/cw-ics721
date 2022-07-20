use cosmwasm_std::StdError;
use cw_utils::ParseReplyError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Only unordered channels are supported.")]
    OrderedChannel {},

    #[error("Invalid IBC channel version. Got ({actual}), expected ({expected}).")]
    InvalidVersion { actual: String, expected: String },

    #[error("ICS 721 channels may not be closed.")]
    CantCloseChannel {},

    #[error("Unrecognised class ID")]
    UnrecognisedClassId {},

    #[error("Class ID already exists")]
    ClassIdAlreadyExists {},

    #[error("Unrecognised reply ID")]
    UnrecognisedReplyId {},

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error("must provide same number of token IDs and URIs")]
    ImbalancedTokenInfo {},
}

/// Enum that can never be constructed. Used as an error type where we
/// can not error.
pub enum Never {}
