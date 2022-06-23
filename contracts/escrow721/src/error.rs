use cosmwasm_std::StdError;
use thiserror::Error;

/// Never is a placeholder to ensure we don't return any errors
#[derive(Error, Debug)]
pub enum Never {}

#[derive(Error, Debug)]
pub enum EscrowContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Invalid reply ID")]
    InvalidReplyID {},

    #[error("Instantiate escrow721 error")]
    InstantiateEscrow721Error {},
}
