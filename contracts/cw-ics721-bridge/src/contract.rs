#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;

use crate::{
    error::ContractError,
    execute::Cw721InitMessage,
    msg::{ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    state::Ics721Contract,
    token_types::Class,
};

const CONTRACT_NAME: &str = "crates.io:cw-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ics721Contract::default().instantiate(deps, env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    Ics721Contract::default().execute(deps, env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    Ics721Contract::default().query(deps, env, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    Ics721Contract::default().migrate(deps, env, msg)
}

impl<'a> Cw721InitMessage for Ics721Contract<'a> {
    fn init_msg(&self, env: &Env, class: &Class) -> StdResult<Binary> {
        to_binary(&cw721_base::msg::InstantiateMsg {
            // Name of the collection MUST be class_id as this is how
            // we create a map entry on reply.
            name: class.id.clone().into(),
            symbol: class.id.clone().into(),
            minter: env.contract.address.to_string(),
        })
    }
}
