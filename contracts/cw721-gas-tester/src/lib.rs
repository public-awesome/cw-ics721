use cosmwasm_schema::cw_serde;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Addr, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw721_base::{msg, ContractError, Extension};
use cw_storage_plus::Item;

pub type ExecuteMsg = msg::ExecuteMsg<Extension, Empty>;
pub type QueryMsg = msg::QueryMsg<Empty>;

#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub minter: String,
    /// An address which will be unable to transfer NFTs away from
    /// themselves (they are a black hole). If this address attempts a
    /// `TransferNft` message it will fail with an out-of-gas error.
    pub target: String,
}

const TARGET: Item<Addr> = Item::new("target");

const CONTRACT_NAME: &str = "crates.io:cw721-gas-tester";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    mut deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    let response = cw721_base::entry::instantiate(
        deps.branch(),
        env,
        info,
        msg::InstantiateMsg {
            name: msg.name,
            symbol: msg.symbol,
            minter: msg.minter,
        },
    )?;
    TARGET.save(deps.storage, &deps.api.addr_validate(&msg.target)?)?;
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    Ok(response)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    if matches!(msg, ExecuteMsg::TransferNft { .. }) && info.sender == TARGET.load(deps.storage)? {
        // loop here causes the relayer to hang while it tries to
        // simulate the TX.
        panic!("gotem")
        // loop {}
    } else {
        cw721_base::entry::execute(deps, env, info, msg)
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    cw721_base::entry::query(deps, env, msg)
}
