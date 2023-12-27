use cosmwasm_schema::cw_serde;
use cosmwasm_std::{to_json_binary, Addr, StdResult, WasmMsg};
use ics721_types::token_types::{Class, Token, TokenId};

use crate::msg::{CallbackMsg, ExecuteMsg};

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
            msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::RedeemVouchers {
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
            msg: to_json_binary(&ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver,
                create: self,
            }))?,
            funds: vec![],
        })
    }
}
