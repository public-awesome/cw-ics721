use cosmwasm_std::IbcTimeout;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {
    /// Code ID of cw721 contract. A new cw721 will be instantiated
    /// for each new IBCd NFT classID.
    pub cw721_code_id: u64,
    /// Code ID for ics-escrow contract. This holds NFTs while they
    /// are away on different chains until they return. A new escrow
    /// is created for each local connection tuple (port, channel).
    pub escrow_code_id: u64,
}

#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Transfer the NFT identified by class_id and token_id to receiver
    Transfer {
        class_id: String,
        token_id: String,
        receiver: String,
    },
    /// Burn the NFT identified by class_id and token_id
    Burn { class_id: String, token_id: String },
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
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns the current owner of the NFT identified by class_id and token_id
    GetOwner { token_id: String, class_id: String },
    /// Returns the NFT identified by class_id and token_id
    GetNft { class_id: String, token_id: String },
    /// Returns true if the NFT class identified by class_id already
    /// exists
    HasClass { class_id: String },
    /// Returns the NFT Class identified by class_id
    GetClass { class_id: String },
    // TODO: Add query for classURI given classID.
}
