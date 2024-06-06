use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{Addr, WasmMsg};
use cw_cii::ContractInstantiateInfo;

use crate::token_types::{VoucherCreation, VoucherRedemption};
use ics721_types::token_types::{Class, ClassId, ClassToken, Token, TokenId};

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
    /// An optional proxy contract. If an incoming proxy is set, the contract
    /// will call it and pass IbcPacket. The proxy is expected
    /// to implement the Ics721ReceiveIbcPacketMsg for message execution.
    pub incoming_proxy: Option<ContractInstantiateInfo>,
    /// An optional proxy contract. If an outging proxy is set, the contract
    /// will only accept NFTs from that proxy. The proxy is expected
    /// to implement the cw721 proxy interface defined in the
    /// cw721-outgoing-proxy crate.
    pub outgoing_proxy: Option<ContractInstantiateInfo>,
    /// Address that may pause the contract. PAUSER may pause the
    /// contract a single time; in pausing the contract they burn the
    /// right to do so again. A new pauser may be later nominated by
    /// the CosmWasm level admin via a migration.
    pub pauser: Option<String>,
    /// The admin address for instantiating new cw721 contracts. In case of None, contract is immutable.
    pub cw721_admin: Option<String>,
    /// The optional contract address length being used for instantiate2. In case of None, default length is 32 (standard in cosmwasm).
    pub contract_addr_length: Option<u32>,
}

#[cw_serde]
pub enum ExecuteMsg {
    /// Receives a NFT to be IBC transfered away. The `msg` field must
    /// be a binary encoded `IbcOutgoingMsg`.
    ReceiveNft(cw721::receiver::Cw721ReceiveMsg),

    /// Pauses the ICS721 contract. Only the pauser may call this. In pausing
    /// the contract, the pauser burns the right to do so again.
    Pause {},

    /// Mesages used internally by the contract. These may only be
    /// called by the contract itself.
    Callback(CallbackMsg),

    /// Admin msg in case something goes wrong.
    /// As a minimum it clean up states (incoming channel and token metadata), and burn NFT if exists.
    AdminCleanAndBurnNft {
        owner: String,
        token_id: String,
        class_id: String,
        collection: String,
    },

    /// Admin msg in case something goes wrong.
    /// As a minimum it clean up state (outgoing channel), and transfer NFT if exists.
    /// - transfer NFT if exists
    AdminCleanAndUnescrowNft {
        recipient: String,
        token_id: String,
        class_id: String,
        collection: String,
    },
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
    /// Redeem all entries in outgoing channel.
    RedeemOutgoingChannelEntries(Vec<(ClassId, TokenId)>),
    /// Save all entries in incoming channel.
    AddIncomingChannelEntries(Vec<((ClassId, TokenId), String)>),
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

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Gets the classID this contract has stored for a given NFT
    /// contract. If there is no class ID for the provided contract,
    /// returns None.
    #[returns(Option<ClassId>)]
    ClassId { contract: String },

    /// Gets the NFT contract associated wtih the provided class
    /// ID. If no such contract exists, returns None. Returns
    /// Option<Addr>.
    #[returns(Option<::cosmwasm_std::Addr>)]
    NftContract { class_id: String },

    /// Returns predictable NFT contract using instantiate2. If no
    /// cw721_code_id is provided, default cw721_code_id from storage is used.
    #[returns(::cosmwasm_std::Addr)]
    GetInstantiate2NftContract {
        class_id: String,
        cw721_code_id: Option<u64>,
    },

    /// Gets the class level metadata URI for the provided
    /// class_id. If there is no metadata, returns None. Returns
    /// `Option<Class>`.
    #[returns(Option<Class>)]
    ClassMetadata { class_id: String },

    #[returns(Option<Token>)]
    TokenMetadata { class_id: String, token_id: String },

    /// Gets the owner of the NFT identified by CLASS_ID and
    /// TOKEN_ID. Errors if no such NFT exists. Returns
    /// `cw721::OwnerOfResonse`.
    #[returns(::cw721::msg::OwnerOfResponse)]
    Owner { class_id: String, token_id: String },

    /// Gets the address that may pause this contract if one is set.
    #[returns(Option<::cosmwasm_std::Addr>)]
    Pauser {},

    /// Gets the current pause status.
    #[returns(bool)]
    Paused {},

    /// Gets this contract's outgoing cw721-outgoing-proxy if one is set.
    #[returns(Option<::cosmwasm_std::Addr>)]
    OutgoingProxy {},

    /// Gets this contract's incoming cw721-outgoing-proxy if one is set.
    #[returns(Option<::cosmwasm_std::Addr>)]
    IncomingProxy {},

    /// Gets the code used for instantiating new cw721s.
    #[returns(u64)]
    Cw721CodeId {},

    /// Gets the admin address for instantiating new cw721 contracts. In case of None, contract is immutable.
    #[returns(Option<Option<::cosmwasm_std::Addr>>)]
    Cw721Admin {},

    /// Gets the contract address length being used for instantiate2. In case of None, default length is 32 (standard in cosmwasm).
    #[returns(Option<u32>)]
    ContractAddrLength {},

    /// Gets a list of classID as key (from
    /// NonFungibleTokenPacketData) and cw721 contract as value
    /// (instantiated for that classID).
    #[returns(Vec<(ClassId, Addr)>)]
    NftContracts {
        start_after: Option<ClassId>,
        limit: Option<u32>,
    },

    /// Gets a list of classID, tokenID, and local channelID. Used
    /// to determine the local channel that NFTs have been sent
    /// out on.
    #[returns(Vec<((ClassId, TokenId), String)>)]
    OutgoingChannels {
        start_after: Option<ClassToken>,
        limit: Option<u32>,
    },

    /// Gets a list of classID, tokenID, and local channel ID. Used
    /// to determine the local channel that NFTs have arrived at
    /// this contract.
    #[returns(Vec<((ClassId, TokenId), String)>)]
    IncomingChannels {
        start_after: Option<ClassToken>,
        limit: Option<u32>,
    },
}

#[cw_serde]
pub enum MigrateMsg {
    WithUpdate {
        /// The address that may pause the contract. If `None` is
        /// provided the current pauser will be removed.
        pauser: Option<String>,
        /// The cw721-outgoing-proxy for this contract. If `None` is provided
        /// the current proxy will be removed.
        outgoing_proxy: Option<String>,
        /// The cw721-outgoing-proxy for this contract. If `None` is provided
        /// the current proxy will be removed.
        incoming_proxy: Option<String>,
        /// Code ID of cw721-ics contract. A new cw721-ics will be
        /// instantiated for each new IBCd NFT classID.
        ///
        /// NOTE: this _must_ correspond to the cw721-base contract. Using
        /// a regular cw721 may cause the ICS 721 interface implemented by
        /// this contract to stop working, and IBCd away NFTs to be
        /// unreturnable as cw721 does not have a mint method in the spec.
        cw721_base_code_id: Option<u64>,
        /// The admin address for instantiating new cw721 contracts. In case of "", contract is immutable.
        cw721_admin: Option<String>,
        /// The optional contract address length being used for instantiate2. In case of None, default length is 32 (standard in cosmwasm).
        contract_addr_length: Option<u32>,
    },
}
