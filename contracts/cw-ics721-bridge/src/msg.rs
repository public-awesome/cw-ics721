use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{to_binary, Addr, Env, IbcTimeout, StdResult, WasmMsg};
use cw721_proxy_derive::cw721_proxy;
use cw_pause_once::UncheckedPausePolicy;

#[cw_serde]
pub struct InstantiateMsg {
    /// Code ID of cw721-ics contract. A new cw721-ics will be
    /// instantiated for each new IBCd NFT classID.
    ///
    /// NOTE: this _must_ correspond to the cw721-base contract. Using
    /// a regular cw721 may cause the ICS 721 interface implemented by
    /// this contract to stop working, and IBCd away NFTs to be
    /// unreturnable (cw721 does not have a mint method in the spec).
    pub cw721_base_code_id: u64,
    /// An optional proxy contract. If a proxy is set the contract
    /// will only accept NFTs from that proxy. The proxy is expected
    /// to implement the cw721 proxy interface defined in the
    /// cw721-proxy crate.
    pub proxy: Option<String>,
    /// Policy for pausing this contract. If policy is none, the
    /// contract will not be pausable. Otherwise, the contract will be
    /// pausable a single time by a prespecified address for a
    /// prespecified duration.
    pub pause_policy: Option<UncheckedPausePolicy>,
}

#[cw721_proxy]
#[cw_serde]
pub enum ExecuteMsg {
    /// Receives a NFT to be IBC transfered away. The `msg` field must
    /// be a binary encoded `IbcAwayMsg`.
    ReceiveNft(cw721::Cw721ReceiveMsg),

    /// Pauses the bridge. Only an address allowed by the pause
    /// policy may execute this method. The bridge will automatically
    /// unpause after duration specified in the pause policy.
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
    /// the classID does not have one yet.
    DoInstantiateAndMint {
        /// The ics721 class ID to mint for.
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
        receiver: String,
        class_uri: Option<String>,
        transfers: Option<TransferInfo>,
        new_tokens: Option<NewTokenInfo>,
    },
}

#[cw_serde]
pub struct TransferInfo {
    pub class_id: String,
    pub token_ids: Vec<String>,
}

#[cw_serde]
pub struct NewTokenInfo {
    pub class_id: String,
    pub token_ids: Vec<String>,
    pub token_uris: Vec<String>,
}

#[cw_serde]
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

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    /// Gets the classID this contract has stored for a given NFT
    /// contract. If there is no class ID for the provided contract,
    /// returns None.
    #[returns(Option<String>)]
    ClassIdForNftContract { contract: String },

    /// Gets the NFT contract associated wtih the provided class
    /// ID. If no such contract exists, returns None. Returns
    /// Option<Addr>.
    #[returns(Option<Addr>)]
    NftContractForClassId { class_id: String },

    /// Gets the class level metadata URI for the provided
    /// class_id. If there is no metadata, returns None. Returns
    /// `Option<String>`.
    #[returns(Option<String>)]
    Metadata { class_id: String },

    /// Gets the owner of the NFT identified by CLASS_ID and
    /// TOKEN_ID. Errors if no such NFT exists. Returns
    /// `cw721::OwnerOfResonse`.
    #[returns(cw721::OwnerOfResponse)]
    Owner { class_id: String, token_id: String },
}

#[cw_serde]
pub enum MigrateMsg {
    WithUpdate {
        pause_policy: Option<UncheckedPausePolicy>,
        proxy: Option<String>,
    },
}

impl TransferInfo {
    pub(crate) fn into_wasm_msg(self, env: &Env, receiver: &Addr) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::BatchTransfer {
                class_id: self.class_id,
                receiver: receiver.to_string(),
                token_ids: self.token_ids,
            }))?,
            funds: vec![],
        })
    }
}

impl NewTokenInfo {
    pub(crate) fn into_wasm_msg(
        self,
        env: &Env,
        receiver: &Addr,
        class_uri: Option<String>,
    ) -> StdResult<WasmMsg> {
        Ok(WasmMsg::Execute {
            contract_addr: env.contract.address.to_string(),
            msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
                class_id: self.class_id,
                class_uri,
                receiver: receiver.to_string(),
                token_ids: self.token_ids,
                token_uris: self.token_uris,
            }))?,
            funds: vec![],
        })
    }
}
