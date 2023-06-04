use cosmwasm_schema::cw_serde;
use cosmwasm_std::WasmMsg;
use cw721_proxy_derive::cw721_proxy;

use crate::token_types::{ClassId, Token, VoucherCreation, VoucherRedemption};

#[cw721_proxy]
#[cw_serde]
pub enum ExecuteMsg {
    /// Receives a NFT to be IBC transfered away. The `msg` field must
    /// be a binary encoded `IbcOutgoingMsg`.
    ReceiveNft(cw721::Cw721ReceiveMsg),

    /// Pauses the bridge. Only the pauser may call this. In pausing
    /// the contract, the pauser burns the right to do so again.
    Pause {},

    /// Mesages used internally by the contract. These may only be
    /// called by the contract itself.
    Callback(CallbackMsg),
}

#[cw_serde]
pub enum CallbackMsg {
    CreateVouchers {
        /// The address that ought to receive the NFT. This is a local
        /// address, not a bech32 public key.
        receiver: String,
        /// Information about the vouchers being created.
        create: VoucherCreation,
    },
    RedeemVouchers {
        /// The address that should receive the tokens.
        receiver: String,
        /// Information about the vouchers been redeemed.
        redeem: VoucherRedemption,
    },
    /// Mints a NFT of collection class_id for receiver with the
    /// provided id and metadata. Only callable by this contract.
    Mint {
        /// The class_id to mint for. This must have previously been
        /// created with `SaveClass`.
        class_id: ClassId,
        /// The address that ought to receive the NFTs. This is a
        /// local address, not a bech32 public key.
        receiver: String,
        /// The tokens to mint on the collection.
        tokens: Vec<Token>,
    },
    /// In submessage terms, say a message that results in an error
    /// "returns false" and one that succedes "returns true". Returns
    /// the logical conjunction (&&) of all the messages in operands.
    ///
    /// Under the hood this just executes them in order. We use this
    /// to respond with a single ACK when a message calls for the
    /// execution of both `CreateVouchers` and `RedeemVouchers`.
    Conjunction { operands: Vec<WasmMsg> },
}
