#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, StdResult};
use cw2::set_contract_version;
use cw_pause_once::PauseOrchestrator;
use cw_storage_plus::{Item, Map};
use ics721::{
    error::ContractError,
    execute::Ics721Execute,
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    query::Ics721Query,
    state::Ics721Contract,
    token_types::{Class, ClassId, TokenId},
};
use sg_std::{Response, StargazeMsgWrapper};

const CONTRACT_NAME: &str = "crates.io:sg-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub struct SgIcs721Contract<'a> {
    /// The code ID we will use for instantiating new cw721s.
    pub cw721_code_id: Item<'a, u64>,
    /// The proxy that this contract is receiving NFTs from, if any.
    pub proxy: Item<'a, Option<Addr>>,
    /// Manages contract pauses.
    pub po: PauseOrchestrator<'a>,

    /// Maps classID (from NonFungibleTokenPacketData) to the cw721
    /// contract we have instantiated for that classID.
    pub class_id_to_nft_contract: Map<'a, ClassId, Addr>,
    /// Maps cw721 contracts to the classID they were instantiated for.
    pub nft_contract_to_class_id: Map<'a, Addr, ClassId>,

    /// Maps between classIDs and classs. We need to keep this state
    /// ourselves as cw721 contracts do not have class-level metadata.
    pub class_id_to_class: Map<'a, ClassId, Class>,

    /// Maps (class ID, token ID) -> local channel ID. Used to determine
    /// the local channel that NFTs have been sent out on.
    pub outgoing_class_token_to_channel: Map<'a, (ClassId, TokenId), String>,
    /// Same as above, but for NFTs arriving at this contract.
    pub incoming_class_token_to_channel: Map<'a, (ClassId, TokenId), String>,
    /// Maps (class ID, token ID) -> token metadata. Used to store
    /// on-chain metadata for tokens that have arrived from other
    /// chains. When a token arrives, it's metadata (regardless of if it
    /// is `None`) is stored in this map. When the token is returned to
    /// it's source chain, the metadata is removed from the map.
    pub token_metadata: Map<'a, (ClassId, TokenId), Option<Binary>>,
}

impl Default for SgIcs721Contract<'static> {
    fn default() -> Self {
        let Ics721Contract {
            cw721_code_id,
            proxy,
            po,
            class_id_to_nft_contract,
            nft_contract_to_class_id,
            class_id_to_class,
            outgoing_class_token_to_channel,
            incoming_class_token_to_channel,
            token_metadata,
        } = Ics721Contract::default();
        Self {
            cw721_code_id,
            proxy,
            po,
            class_id_to_nft_contract,
            nft_contract_to_class_id,
            class_id_to_class,
            outgoing_class_token_to_channel,
            incoming_class_token_to_channel,
            token_metadata,
        }
    }
}

impl Ics721Execute<StargazeMsgWrapper> for SgIcs721Contract<'static> {
    fn init_msg(&self, env: &Env, class: &Class) -> StdResult<Binary> {
        to_binary(&sg721::InstantiateMsg {
            // Name of the collection MUST be class_id as this is how
            // we create a map entry on reply.
            name: class.id.clone().into(),
            symbol: class.id.clone().into(),
            minter: env.contract.address.to_string(),
            collection_info: sg721::CollectionInfo {
                creator: env.contract.address.to_string(),
                description: "".to_string(),
                image: "https://arkprotocol.io".to_string(),
                external_link: None,
                explicit_content: None,
                start_trading_time: None,
                royalty_info: None,
            },
        })
    }
}

impl Ics721Query for SgIcs721Contract<'static> {}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    SgIcs721Contract::default().instantiate(deps, env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    SgIcs721Contract::default().execute(deps, env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    SgIcs721Contract::default().query(deps, env, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    SgIcs721Contract::default().migrate(deps, env, msg)
}

#[cfg(test)]
mod testing;
