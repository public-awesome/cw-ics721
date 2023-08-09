use std::ops::Deref;

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_binary, Addr, Binary, StdResult, WasmMsg};
use cw_storage_plus::{Bound, Bounder, Key, KeyDeserialize, Prefixer, PrimaryKey};

use crate::msg::{CallbackMsg, ExecuteMsg};

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

#[cw_serde]
pub struct VoucherRedemption {
    /// The class that these vouchers are being redeemed from.
    pub class: Class,
    /// The tokens belonging to `class` that ought to be redeemed.
    pub token_ids: Vec<TokenId>,
}

#[cw_serde]
pub struct VoucherCreation {
    /// The class that these vouchers are being created for.
    pub class: Class,
    /// The tokens to create debt-vouchers for.
    pub tokens: Vec<Token>,
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

impl<'a> Bounder<'a> for ClassId {
    fn inclusive_bound(self) -> Option<cw_storage_plus::Bound<'a, Self>> {
        Some(Bound::inclusive(self))
    }

    fn exclusive_bound(self) -> Option<cw_storage_plus::Bound<'a, Self>> {
        Some(Bound::exclusive(self))
    }
}

impl VoucherRedemption {
    /// Transforms information about a redemption of vouchers into a
    /// message that may be executed to redeem said vouchers.
    ///
    /// ## Arguments
    ///
    /// - `contract` the address of the ics721 contract
    ///   vouchers are being redeemed on.
    /// - `receiver` that address that ought to receive the NFTs the
    ///   debt-vouchers are redeemable for.
    pub(crate) fn into_wasm_msg(self, contract: Addr, receiver: String) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: contract.into_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::RedeemVouchers {
                receiver,
                redeem: self,
            }))?,
            funds: vec![],
        })
    }
}

impl VoucherCreation {
    /// Transforms information abiout the creation of vouchers into a
    /// message that may be executed to redeem said vouchers.
    ///
    /// ## Arguments
    ///
    /// - `contract` the address of the ics721 contract
    ///   vouchers are being created on.
    /// - `receiver` that address that ought to receive the newly
    ///   created debt-vouchers.
    pub(crate) fn into_wasm_msg(self, contract: Addr, receiver: String) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: contract.into_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver,
                create: self,
            }))?,
            funds: vec![],
        })
    }
}

// boilerplate for conversion between wrappers and the wrapped.

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

// boilerplate for storing these wrapper types in CosmWasm maps.

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
