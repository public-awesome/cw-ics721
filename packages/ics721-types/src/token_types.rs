use std::ops::Deref;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Binary, StdResult};
use cw_storage_plus::{Bound, Bounder, Key, KeyDeserialize, Prefixer, PrimaryKey};

/// A token ID according to the ICS-721 spec. The newtype pattern is
/// used here to provide some distinction between token and class IDs
/// in the type system.
#[cw_serde]
pub struct TokenId(String);

/// A token according to the ICS-721 spec.
#[cw_serde]
pub struct Token {
    /// A unique identifier for the token.
    pub id: TokenId,
    /// Optional URI pointing to off-chain metadata about the token.
    pub uri: Option<String>,
    /// Optional base64 encoded metadata about the token.
    pub data: Option<Binary>,
}

/// A class ID according to the ICS-721 spec. The newtype pattern is
/// used here to provide some distinction between token and class IDs
/// in the type system.
#[cw_serde]
pub struct ClassId(String);

#[cw_serde]
pub struct Class {
    /// A unique (from the source chain's perspective) identifier for
    /// the class.
    pub id: ClassId,
    /// Optional URI pointing to off-chain metadata about the class.
    pub uri: Option<String>,
    /// Optional base64 encoded metadata about the class.
    pub data: Option<Binary>,
}

impl TokenId {
    pub fn new<T>(token_id: T) -> Self
    where
        T: Into<String>,
    {
        Self(token_id.into())
    }
}

impl ClassId {
    pub fn new<T>(class_id: T) -> Self
    where
        T: Into<String>,
    {
        Self(class_id.into())
    }
}

#[cw_serde]
pub struct ClassToken {
    pub class_id: ClassId,
    pub token_id: TokenId,
}

impl<'a> Bounder<'a> for ClassId {
    fn inclusive_bound(self) -> Option<cw_storage_plus::Bound<'a, Self>> {
        Some(Bound::inclusive(self))
    }

    fn exclusive_bound(self) -> Option<cw_storage_plus::Bound<'a, Self>> {
        Some(Bound::exclusive(self))
    }
}

// Allow ClassId to be inferred into String
impl From<ClassId> for String {
    fn from(c: ClassId) -> Self {
        c.0
    }
}

impl From<TokenId> for String {
    fn from(t: TokenId) -> Self {
        t.0
    }
}

impl Deref for ClassId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::fmt::Display for ClassId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// Allow ClassId to be inferred into Key - using String.key()
impl<'a> PrimaryKey<'a> for ClassId {
    type Prefix = <String as PrimaryKey<'a>>::Prefix;
    type SubPrefix = <String as PrimaryKey<'a>>::SubPrefix;
    type Suffix = <String as PrimaryKey<'a>>::Suffix;
    type SuperSuffix = <String as PrimaryKey<'a>>::SuperSuffix;

    fn key(&self) -> Vec<cw_storage_plus::Key> {
        self.0.key()
    }
}

impl<'a> PrimaryKey<'a> for TokenId {
    type Prefix = <String as PrimaryKey<'a>>::Prefix;
    type SubPrefix = <String as PrimaryKey<'a>>::SubPrefix;
    type Suffix = <String as PrimaryKey<'a>>::Suffix;
    type SuperSuffix = <String as PrimaryKey<'a>>::SuperSuffix;

    fn key(&self) -> Vec<cw_storage_plus::Key> {
        self.0.key()
    }
}

impl<'a> Prefixer<'a> for ClassId {
    fn prefix(&self) -> Vec<Key> {
        self.0.prefix()
    }
}

impl<'a> Prefixer<'a> for TokenId {
    fn prefix(&self) -> Vec<Key> {
        self.0.prefix()
    }
}

impl KeyDeserialize for ClassId {
    type Output = <String as KeyDeserialize>::Output;
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        String::from_vec(value)
    }
}

impl KeyDeserialize for TokenId {
    type Output = <String as KeyDeserialize>::Output;
    fn from_vec(value: Vec<u8>) -> StdResult<Self::Output> {
        String::from_vec(value)
    }
}
