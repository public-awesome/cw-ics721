use cosmwasm_schema::cw_serde;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw721_base::{
    error::ContractError, msg, DefaultOptionalCollectionExtension,
    DefaultOptionalCollectionExtensionMsg, DefaultOptionalNftExtension,
    DefaultOptionalNftExtensionMsg,
};
use cw_storage_plus::Item;

pub type ExecuteMsg =
    msg::ExecuteMsg<DefaultOptionalNftExtensionMsg, DefaultOptionalCollectionExtensionMsg, Empty>;
pub type QueryMsg =
    msg::QueryMsg<DefaultOptionalNftExtension, DefaultOptionalCollectionExtension, Empty>;

#[cw_serde]
pub struct InstantiateMsg {
    pub name: String,
    pub symbol: String,
    pub minter: String,
    /// An address which will be unable receive NFT on `TransferNft` message
    /// If `TransferNft` message attempts sending to banned recipient
    /// it will fail with an out-of-gas error.
    pub banned_recipient: String,
}

const BANNED_RECIPIENT: Item<String> = Item::new("banned_recipient");

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
        msg::InstantiateMsg::<DefaultOptionalCollectionExtensionMsg> {
            name: msg.name,
            symbol: msg.symbol,
            minter: Some(msg.minter),
            creator: None,
            collection_info_extension: None,
            withdraw_address: None,
        },
    )?;
    BANNED_RECIPIENT.save(deps.storage, &msg.banned_recipient)?;
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
    match msg.clone() {
        ExecuteMsg::TransferNft { recipient, .. } => {
            if recipient == BANNED_RECIPIENT.load(deps.storage)? {
                // loop here causes the relayer to hang while it tries to
                // simulate the TX.
                panic!("gotem")
                // loop {}
            }
            cw721_base::entry::execute(deps, env, info, msg)
        }
        _ => cw721_base::entry::execute(deps, env, info, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    cw721_base::entry::query(deps, env, msg)
}
