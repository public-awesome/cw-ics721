use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Binary, IbcTimeout};
use cw721_proxy_derive::cw721_proxy;
use cw_cii::ContractInstantiateInfo;

#[cw_serde]
pub struct InstantiateMsg {
    /// Code ID of cw721-ics contract. A new cw721-ics will be
    /// instantiated for each new IBCd NFT classID.
    ///
    /// NOTE: this _must_ correspond to the cw721-base contract. Using
    /// a regular cw721 may cause the ICS 721 interface implemented by
    /// this contract to stop working, and IBCd away NFTs to be
    /// unreturnable as cw721 does not have a mint method in the spec.
    pub cw721_base_code_id: u64,
    /// An optional proxy contract. If a proxy is set the contract
    /// will only accept NFTs from that proxy. The proxy is expected
    /// to implement the cw721 proxy interface defined in the
    /// cw721-proxy crate.
    pub proxy: Option<ContractInstantiateInfo>,
    /// Address that may pause the contract. PAUSER may pause the
    /// contract a single time; in pausing the contract they burn the
    /// right to do so again. A new pauser may be later nominated by
    /// the CosmWasm level admin via a migration.
    pub pauser: Option<String>,
}

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
    /// Mints a NFT of collection class_id for receiver with the
    /// provided id and metadata. Only callable by this contract.
    Mint {
        /// The class_id to mint for. This must have previously been
        /// created with `SaveClass`.
        class_id: String,
        /// The tokens to be minted.
        tokens: Vec<Token>,
        /// The address that ought to receive the NFTs. This is a
        /// local address, not a bech32 public key.
        receiver: String,
    },
    /// Much like mint, but will instantiate a new cw721 contract iff
    /// the classID does not have one yet.
    InstantiateAndMint {
        /// The ics721 class ID to mint for.
        class_id: String,
        /// The URI for this class ID.
        class_uri: String,
        /// The on-chain metadata for this collection.
        class_data: Binary,
        /// The tokens to create.
        tokens: Vec<Token>,
        /// The address that ought to receive the NFTs. This is a
        /// local address, not a bech32 public key.
        receiver: String,
    },
    /// Transfers a number of NFTs identified by CLASS_ID and
    /// TOKEN_IDS to RECEIVER.
    BatchTransfer {
        /// The ics721 class ID of the tokens to be transfered.
        class_id: String,
        /// The address that should receive the tokens.
        receiver: String,
        /// The tokens (of CLASS_ID) that should be sent.
        token_ids: Vec<String>,
    },
    /// Handles the falliable part of receiving an IBC
    /// packet. Transforms TRANSFERS into a `BatchTransfer` message
    /// and NEW_TOKENS into a `DoInstantiateAndMint`, then dispatches
    /// those methods.
    HandlePacketReceive {
        /// The address receiving the NFTs.
        receiver: String,
        /// The metadata URI for the collection being transfered.
        class_uri: String,
        /// On-chain metadata for the collection being transfered.
        class_data: Binary,
        /// Information about transfer actions.
        transfers: Option<TransferInfo>,
        /// Information about mint actions.
        new_tokens: Option<NewTokenInfo>,
    },
}

#[cw_serde]
pub struct TransferInfo {
    /// The class ID the tokens belong to.
    pub class_id: String,
    /// The tokens to be transfered.
    pub token_ids: Vec<String>,
}

/// An ICS-721 token and associated metadata.
#[cw_serde]
pub struct Token {
    pub token_id: String,
    pub token_uri: String,
    pub token_data: Binary,
}

#[cw_serde]
pub struct NewTokenInfo {
    /// The class ID to mint tokens for.
    pub class_id: String,
    /// Tokens to create.
    pub tokens: Vec<Token>,
}

#[cw_serde]
pub struct IbcOutgoingMsg {
    /// The address that should receive the NFT being sent on the
    /// *receiving chain*.
    pub receiver: String,
    /// A memo to include with the NFT transfer.
    pub memo: Option<String>,
    /// The *local* channel ID this ought to be sent away on. This
    /// contract must have a connection on this channel.
    pub channel_id: String,
    /// Timeout for the IBC message.
    pub timeout: IbcTimeout,
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Gets the classID this contract has stored for a given NFT
    /// contract. If there is no class ID for the provided contract,
    /// returns None.
    #[returns(Option<String>)]
    ClassId { contract: String },

    /// Gets the NFT contract associated wtih the provided class
    /// ID. If no such contract exists, returns None. Returns
    /// Option<Addr>.
    #[returns(Option<::cosmwasm_std::Addr>)]
    NftContract { class_id: String },

    /// Gets the class level metadata for the provided class_id.
    #[returns(crate::state::CollectionMetadata)]
    CollectionMetadata { class_id: String },

    /// Gets the on-chain metadata for the provided token. If no
    /// metadata has been specified, `cosmwasm_std::Binary::default()`
    /// is returned.
    #[returns(::cosmwasm_std::Binary)]
    TokenMetadata { class_id: String, token_id: String },

    /// Gets the owner of the NFT identified by CLASS_ID and
    /// TOKEN_ID. Errors if no such NFT exists. Returns
    /// `cw721::OwnerOfResonse`.
    #[returns(::cw721::OwnerOfResponse)]
    Owner { class_id: String, token_id: String },

    /// Gets the address that may pause this contract if one is set.
    #[returns(Option<::cosmwasm_std::Addr>)]
    Pauser {},

    /// Gets the current pause status.
    #[returns(bool)]
    Paused {},

    /// Gets this contract's cw721-proxy if one is set.
    #[returns(Option<::cosmwasm_std::Addr>)]
    Proxy {},
}

#[cw_serde]
pub enum MigrateMsg {
    WithUpdate {
        /// The address that may pause the contract. If `None` is
        /// provided the current pauser will be removed.
        pauser: Option<String>,
        /// The cw721-proxy for this contract. If `None` is provided
        /// the current proxy will be removed.
        proxy: Option<String>,
    },
}
