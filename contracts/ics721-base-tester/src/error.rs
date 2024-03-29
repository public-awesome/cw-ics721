use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Never {}

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("only unordered channels are supported")]
    OrderedChannel {},

    #[error("invalid IBC channel version. Got ({actual}), expected ({expected})")]
    InvalidVersion { actual: String, expected: String },

    #[error("{what}")]
    Debug { what: String },

    #[error("Just some random error")]
    RandomError,

    #[error("Invalid callback")]
    InvalidCallback,

    #[error("The callback sender is not the ics721")]
    SenderIsNotIcs721,
}
