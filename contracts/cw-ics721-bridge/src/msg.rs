use cosmwasm_std::IbcTimeout;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
#[cfg_attr(test, derive(Debug, Clone))]
pub struct InstantiateMsg {
    /// Code ID of cw721-ics contract. A new cw721-ics will be
    /// instantiated for each new IBCd NFT classID.
    ///
    /// NOTE: this _must_ correspond to the cw721-ics contract which
    /// should have been distributed along with this sourcecode. Using
    /// a regular cw721 may cause the ICS 721 interface implemented by
    /// this contract to stop working, and IBCd away NFTs to be
    /// unreturnable (cw721 does not have a burn method in the spec).
    pub cw721_base_code_id: u64,
    /// Code ID for ics-escrow contract. This holds NFTs while they
    /// are away on different chains until they return. A new escrow
    /// is created for each local connection tuple (port, channel).
    pub escrow_code_id: u64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[cfg_attr(test, derive(Debug, Clone))]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Mints a NFT of collection class_id for receiver with the
    /// provided id and metadata. Only callable by this contract.
    Mint {
        /// The class_id to mint for. This must have previously been
        /// created with `SaveClass`.
        class_id: String,
        /// Unique identifiers for the tokens.
        token_ids: Vec<String>,
        /// Urls pointing to metadata about the NFTs to mint. For
        /// example, this may point to ERC721 metadata on IPFS. Must
        /// be the same length as token_ids. token_uris[i] is the
        /// metadata for token_ids[i].
        token_uris: Vec<String>,
        /// The address that ought to receive the NFTs. This is a
        /// local address, not a bech32 public key.
        receiver: String,
    },
    /// Much like mint, but will instantiate a new cw721 contract iff
    /// the classID does not have one yet. Needed because we can only
    /// dispatch one submessage at a time from `ibc_packet_receive`
    /// and properly handle IBC error handling. Only callable by this
    /// contract.
    DoInstantiateAndMint {
        /// The class_id to mint for. This must have previously been
        /// created with `SaveClass`.
        class_id: String,
        /// The URI for this class ID.
        class_uri: Option<String>,
        /// Unique identifiers for the tokens being transfered.
        token_ids: Vec<String>,
        /// A list of urls pointing to metadata about the NFTs. For
        /// example, this may point to ERC721 metadata on ipfs.
        ///
        /// Must be the same length as token_ids.
        token_uris: Vec<String>,
        /// The address that ought to receive the NFT. This is a local
        /// address, not a bech32 public key.
        receiver: String,
    },
    /// Receives a NFT to be IBC transfered away. The `msg` field must
    /// be a binary encoded `IbcAwayMsg`.
    ReceiveNft(cw721::Cw721ReceiveMsg),
    /// Transfers a group of NFTs from the escrow for a the given
    /// channel. Callable only by the contract.
    BatchTransferFromChannel {
        channel: String,
        class_id: String,
        token_ids: Vec<String>,
        receiver: String,
    },
    /// Burns the specified tokens that are inside the escrow for the
    /// specified channel. Only callable by this contract.
    BurnEscrowTokens {
        channel: String,
        class_id: String,
        token_ids: Vec<String>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct IbcAwayMsg {
    /// The address that should receive the NFT being sent on the
    /// *receiving chain*.
    pub receiver: String,
    /// The *local* channel ID this ought to be sent away on. This
    /// contract must have a connection on this channel.
    pub channel_id: String,
    /// Timeout for the IBC message. TODO: make this optional and set
    /// default?
    pub timeout: IbcTimeout,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[cfg_attr(test, derive(Debug, Clone))]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns the current owner of the NFT identified by class_id
    /// and token_id. Returns `cw721::OwnerOfResponse`.
    GetOwner { token_id: String, class_id: String },
    /// Returns the NFT identified by class_id and token_id. Returns
    /// `cw721::NftInfoResponse`.
    GetNft { class_id: String, token_id: String },
    /// Returns true if the NFT class identified by class_id already
    /// exists (it has been received). Returns bool.
    HasClass { class_id: String },
    /// Returns the NFT contract identified by class_id. Returns
    /// `Addr`.
    GetClass { class_id: String },
    /// Gets the class level metadata URI for the provided
    /// class_id. Returns GetUriResponse.
    GetUri { class_id: String },

    /// Paginated query over all the channels this contract is
    /// connected to. Returns `Vec<ChannelInfoResponse>`.
    ListChannels {
        start_after: Option<String>,
        limit: Option<u32>,
    },
    /// Paginated query over all the class IDs this contract has seen
    /// and their associated cw721 contracts. Returns
    /// `Vec<ClassIdInfoResponse>`.
    ListClassIds {
        start_after: Option<String>,
        limit: Option<u32>,
    },
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ChannelInfoResponse {
    pub channel_id: String,
    pub escrow_addr: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct ClassIdInfoResponse {
    pub class_id: String,
    pub cw721_addr: String,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct GetUriResponse {
    pub uri: Option<String>,
}
