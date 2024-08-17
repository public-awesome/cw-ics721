use cosmwasm_schema::cw_serde;
#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response};
use cw2::set_contract_version;
use cw721::{
    error::Cw721ContractError,
    msg::Cw721InstantiateMsg,
    traits::{Cw721Execute, Cw721Query},
    DefaultOptionalCollectionExtensionMsg,
};
use cw721_metadata_onchain::Cw721MetadataContract;
use cw_storage_plus::Item;

pub type ExecuteMsg = cw721_metadata_onchain::msg::ExecuteMsg;
pub type QueryMsg = cw721_metadata_onchain::msg::QueryMsg;

#[cw_serde]
pub struct InstantiateMsg {
    /// Name of the NFT contract
    pub name: String,
    /// Symbol of the NFT contract
    pub symbol: String,
    /// Optional extension of the collection metadata
    pub collection_info_extension: DefaultOptionalCollectionExtensionMsg,

    /// The minter is the only one who can create new NFTs.
    /// This is designed for a base NFT that is controlled by an external program
    /// or contract. You will likely replace this with custom logic in custom NFTs
    pub minter: Option<String>,

    /// Sets the creator of collection. The creator is the only one eligible to update `CollectionInfo`.
    pub creator: Option<String>,

    pub withdraw_address: Option<String>,
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
) -> Result<Response, Cw721ContractError> {
    let response = Cw721MetadataContract::default().instantiate_with_version(
        deps.branch(),
        &env,
        &info,
        Cw721InstantiateMsg {
            name: msg.name,
            symbol: msg.symbol,
            minter: msg.minter,
            withdraw_address: None,
            collection_info_extension: msg.collection_info_extension,
            creator: msg.creator,
        },
        CONTRACT_NAME,
        CONTRACT_VERSION,
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
) -> Result<Response, Cw721ContractError> {
    match msg.clone() {
        ExecuteMsg::TransferNft { recipient, .. } => {
            if recipient == BANNED_RECIPIENT.load(deps.storage)? {
                // loop here causes the relayer to hang while it tries to
                // simulate the TX.
                panic!("gotem")
                // loop {}
            }
            Cw721MetadataContract::default().execute(deps, &env, &info, msg)
        }
        _ => Cw721MetadataContract::default().execute(deps, &env, &info, msg),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, Cw721ContractError> {
    Cw721MetadataContract::default().query(deps, &env, msg)
}
