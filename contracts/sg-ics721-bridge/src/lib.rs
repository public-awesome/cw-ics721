#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, StdResult};
use cw2::set_contract_version;
use cw_pause_once::PauseOrchestrator;
use cw_storage_plus::Item;
use ics721::{
    error::ContractError,
    execute::Ics721Execute,
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    query::Ics721Query,
    state::{ChannelsInfo, ClassIdInfo, Cw721Info, Ics721Contract},
    token_types::Class,
};
use sg_std::{Response, StargazeMsgWrapper};

const CONTRACT_NAME: &str = "crates.io:sg-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// This type is an exact copy of Ics721Contract, since only traits defined in the current crate
// can be implemented for types defined outside of the crate.
pub struct SgIcs721Contract<'a> {
    /// The proxy that this contract is receiving NFTs from, if any.
    pub proxy: Item<'a, Option<Addr>>,
    /// Manages contract pauses.
    pub po: PauseOrchestrator<'a>,
    /// cw721 related info like code ID and token metadata.
    pub cw721_info: Cw721Info<'a>,
    /// cw721 class related info like class ID to cw721 contract mappings.
    pub class_id_info: ClassIdInfo<'a>,
    /// Maps (class ID, token ID) -> to local incoming and outgoing channel ID.
    pub channels_info: ChannelsInfo<'a>,
}

impl Default for SgIcs721Contract<'static> {
    fn default() -> Self {
        let Ics721Contract {
            proxy,
            po,
            cw721_info,
            class_id_info,
            channels_info,
        } = Ics721Contract::default();
        Self {
            proxy,
            po,
            cw721_info,
            class_id_info,
            channels_info,
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
