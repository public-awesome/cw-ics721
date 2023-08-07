#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Addr, Binary, Deps, DepsMut, Env, MessageInfo, Order, Response, StdResult,
};
use cw2::set_contract_version;
use cw_storage_plus::Map;

use crate::{
    error::ContractError,
    msg::{ClassToken, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    state::{Ics721Contract, UniversalAllNftInfoResponse},
    token_types::{Class, ClassId, Token, TokenId},
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
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    match msg {
        MigrateMsg::WithUpdate { pauser, proxy } => {
            Ics721Contract::default().proxy.save(
                deps.storage,
                &proxy
                    .as_ref()
                    .map(|h| deps.api.addr_validate(h))
                    .transpose()?,
            )?;
            Ics721Contract::default()
                .po
                .set_pauser(deps.storage, deps.api, pauser.as_deref())?;
            Ok(Response::default().add_attribute("method", "migrate"))
        }
    }
}
