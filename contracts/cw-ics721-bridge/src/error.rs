use cosmwasm_std::StdError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("Unauthorized")]
    Unauthorized {},

    #[error("Only unordered channels are supported.")]
    OrderedChannel {},

    #[error("Invalid IBC channel version. Got ({actual}), expected ({expected}).")]
    InvalidVersion { actual: String, expected: String },

    #[error("ICS 721 channels may not be closed.")]
    CantCloseChannel {},
}
