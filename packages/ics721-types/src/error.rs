use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ValidationError {
    #[error("empty class ID")]
    EmptyClassId {},

    #[error("must transfer at least one token")]
    NoTokens {},

    #[error("optional fields may not be empty if provided")]
    EmptyOptional {},

    #[error("tokenIds, tokenUris, and tokenData must have the same length")]
    TokenInfoLenMissmatch {},
}
