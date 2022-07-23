#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{ADMIN_ADDRESS, CHANNEL_ID};

const CONTRACT_NAME: &str = "crates.io:ics-escrow";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    let admin_address = deps.api.addr_validate(&msg.admin_address)?;
    ADMIN_ADDRESS.save(deps.storage, &admin_address)?;
    CHANNEL_ID.save(deps.storage, &msg.channel_id)?;
    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("admin_address", admin_address.to_string())
        .add_attribute("channel_id", msg.channel_id))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Withdraw {
            nft_address,
            token_id,
            receiver,
        } => execute_withdraw(deps, env, info, nft_address, token_id, receiver),
        ExecuteMsg::Burn {
            nft_address,
            token_ids,
        } => execute_burn(deps.as_ref(), info, nft_address, token_ids),
    }
}

fn execute_withdraw(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    nft_address: String,
    token_id: String,
    receiver: String,
) -> Result<Response, ContractError> {
    let admin_address = ADMIN_ADDRESS.load(deps.storage)?;
    if info.sender != admin_address {
        return Err(ContractError::Unauthorized {});
    }

    // Validate receiver and class_uri
    deps.api.addr_validate(&nft_address)?;
    deps.api.addr_validate(&receiver)?;

    let transfer_msg = cw721::Cw721ExecuteMsg::TransferNft {
        recipient: receiver,
        token_id,
    };
    let msg = WasmMsg::Execute {
        contract_addr: nft_address,
        msg: to_binary(&transfer_msg)?,
        funds: vec![],
    };
    Ok(Response::new()
        .add_attribute("action", "withdraw")
        .add_message(msg))
}

fn execute_burn(
    deps: Deps,
    info: MessageInfo,
    nft_address: String,
    token_ids: Vec<String>,
) -> Result<Response, ContractError> {
    let admin_address = ADMIN_ADDRESS.load(deps.storage)?;
    if info.sender != admin_address {
        return Err(ContractError::Unauthorized {});
    }
    deps.api.addr_validate(&nft_address)?;

    let messages = token_ids
        .into_iter()
        .map(|token_id: String| -> StdResult<WasmMsg> {
            Ok(WasmMsg::Execute {
                contract_addr: nft_address.clone(),
                // This works despite the fact that we need to be
                // compatible with the cw721 base spec (which does not
                // have a burn method) everywhere else.
                //
                // This reason is wrapped up in how this whole machine
                // works. For NFTs that are coming in from an external
                // chain, the bridge contract has control over what
                // cw721 contract is instantiated for them. It could,
                // for example, choose to instantiate ones that point
                // only to images of purple squares. In our case, we
                // choose to instantiate ones with a burn method.
                //
                // A well behaved ICS721 contract will only ever burn
                // NFTs it has minted in response to a foriegn chain's
                // sending them over. There are reasons for this
                // technically, but from a higher level this makes
                // some sense. It wouldn't make sense if a
                // bidirectional bridge burned your NFT.
                //
                // As we mint cw721s with a burn method, and we only
                // burn NFTs that we have minted, this works.
                msg: to_binary(&cw721_base::msg::ExecuteMsg::<Empty>::Burn { token_id })?,
                funds: vec![],
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(Response::new()
        .add_attribute("method", "execute_burn")
        .add_messages(messages))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::AdminAddress {} => to_binary(&ADMIN_ADDRESS.load(deps.storage)?),
        QueryMsg::ChannelId {} => to_binary(&CHANNEL_ID.load(deps.storage)?),
    }
}
