use cosmwasm_std::{Instantiate2AddressError, StdError};
use cw_pause_once::PauseError;
use cw_utils::ParseReplyError;
use ics721_types::error::ValidationError;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    ValidationError(#[from] ValidationError),

    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    Pause(#[from] PauseError),

    #[error(transparent)]
    Instantiate2Error(#[from] Instantiate2AddressError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("NFT not escrowed by ICS721! Owner: {0}")]
    NotEscrowedByIcs721(String),

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

    #[error("Transfer contains both redemption and a creation action")]
    InvalidTransferBothActions,

    #[error("Transfer Doesn't contain any action, no redemption or creation")]
    InvalidTransferNoAction,

    #[error("Couldn't find nft contract for this class id: {0}")]
    NoNftContractForClassId(String),
}
