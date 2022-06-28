use cosmwasm_std::StdError;
use cw_utils::PaymentError;
use thiserror::Error;

/// Never is a placeholder to ensure we don't return any errors
#[derive(Error, Debug)]
pub enum Never {}

pub const ERROR_ESCROW_MAP_SAVE: &str = "Error on escrow map save";
pub const ERROR_INSTANTIATE_ESCROW_REPLY: &str = "Error on instantiate escrow contract parse reply";

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("{0}")]
    Payment(#[from] PaymentError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("UnknownReplyID")]
    UnknownReplyId { id: u64 },

    #[error("NoSuchChannel")]
    NoSuchChannel { id: String },

    #[error("Only supports channel with ibc version ics721-1, got {version}")]
    InvalidIbcVersion { version: String },

    #[error("Only supports unordered channel")]
    OnlyOrderedChannel {},

    #[error("Only accepts tokens that originate on this chain, not native tokens of remote chain")]
    NoForeignTokens {},

    #[error("Parsed port from denom ({port}) doesn't match packet")]
    FromOtherPort { port: String },

    #[error("Parsed channel from denom ({channel}) doesn't match packet")]
    FromOtherChannel { channel: String },

    #[error("NoSuchNft")]
    NoSuchNft { class_id: String },

    #[error("Invalid reply ID")]
    InvalidReplyID {},

    #[error("Instantiate escrow721 error")]
    InstantiateEscrow721Error { msg: String },
}
