use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::Addr;
use cw_cii::ContractInstantiateInfo;
use ics721::token_types::{ClassId, TokenId};

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

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Gets the classID this contract has stored for a given NFT
    /// contract. If there is no class ID for the provided contract,
    /// returns None.
    #[returns(Option<ics721::token_types::ClassId>)]
    ClassId { contract: String },

    /// Gets the NFT contract associated wtih the provided class
    /// ID. If no such contract exists, returns None. Returns
    /// Option<Addr>.
    #[returns(Option<::cosmwasm_std::Addr>)]
    NftContract { class_id: String },

    /// Gets the class level metadata URI for the provided
    /// class_id. If there is no metadata, returns None. Returns
    /// `Option<Class>`.
    #[returns(Option<ics721::token_types::Class>)]
    ClassMetadata { class_id: String },

    #[returns(Option<ics721::token_types::Token>)]
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

    /// Gets the code used for instantiating new cw721s.
    #[returns(u64)]
    Cw721CodeId {},

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
pub struct ClassToken {
    pub class_id: ClassId,
    pub token_id: TokenId,
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
