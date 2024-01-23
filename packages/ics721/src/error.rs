use cosmwasm_std::{Binary, Instantiate2AddressError, StdError};
use cw_pause_once::PauseError;
use cw_utils::ParseReplyError;
use ics721_types::error::Ics721Error;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error(transparent)]
    Ics721Error(#[from] Ics721Error),

    #[error(transparent)]
    Std(#[from] StdError),

    #[error(transparent)]
    Pause(#[from] PauseError),

    #[error(transparent)]
    Instantiate2Error(#[from] Instantiate2AddressError),

    #[error("unauthorized")]
    Unauthorized {},

    #[error("unauthorized")]
    UnknownMsg(Binary),

    #[error("NFT not escrowed by ICS721! Owner: {0}")]
    NotEscrowedByIcs721(String),

    #[error("only unordered channels are supported")]
    OrderedChannel {},

    #[error("invalid IBC channel version - got ({actual}), expected ({expected})")]
    InvalidVersion { actual: String, expected: String },

    #[error("ICS 721 channels may not be closed")]
    CantCloseChannel {},

    #[error("unrecognised reply ID")]
    UnrecognisedReplyId {},

    #[error(transparent)]
    ParseReplyError(#[from] ParseReplyError),

    #[error("Transfer contains both redemption and a creation action")]
    InvalidTransferBothActions,

    #[error("Transfer Doesn't contain any action, no redemption or creation")]
    InvalidTransferNoAction,

    #[error("Couldn't find nft contract for class id: {0}")]
    NoNftContractForClassId(String),

    #[error("Couldn't find class id for nft contract: {0}")]
    NoClassIdForNftContract(String),
}
