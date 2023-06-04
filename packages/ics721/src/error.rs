use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum Ics721Error {
    #[error("optional fields may not be empty if provided")]
    EmptyOptional,

    #[error("empty class ID")]
    EmptyClassId,

    #[error("must transfer at least one token")]
    NoTokens,

    #[error("tokenIds, tokenUris, and tokenData must have the same length")]
    TokenInfoLenMissmatch,
}
