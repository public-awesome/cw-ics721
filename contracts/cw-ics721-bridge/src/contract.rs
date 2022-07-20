#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, Response, StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::helpers::{
    burn, get_class, get_nft, get_owner, has_class, transfer, INSTANTIATE_CW721_REPLY_ID,
};
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{CLASS_ID_TO_NFT_CONTRACT, CW721_CODE_ID};

const CONTRACT_NAME: &str = "crates.io:cw-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    _msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    todo!()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::Transfer {
            class_id,
            token_id,
            receiver,
        } => execute_transfer(deps, env, info, class_id, token_id, receiver),
        ExecuteMsg::Burn { class_id, token_id } => {
            execute_burn(deps, env, info, class_id, token_id)
        }
        ExecuteMsg::Mint {
            class_id,
            token_ids,
            token_uris,
            receiver,
        } => execute_mint(
            deps.as_ref(),
            env,
            info,
            class_id,
            token_ids,
            token_uris,
            receiver,
        ),
        ExecuteMsg::DoInstantiateAndMint {
            class_id,
            token_ids,
            token_uris,
            receiver,
        } => execute_do_instantiate_and_mint(
            deps.as_ref(),
            env,
            info,
            class_id,
            token_ids,
            token_uris,
            receiver,
        ),
    }
}

fn execute_transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
    receiver: String,
) -> Result<Response, ContractError> {
    // This will error if the class_id does not exist so no need to check
    let owner = get_owner(deps.as_ref(), class_id.clone(), token_id.clone())?;

    // Check if we are the owner or the contract itself
    if info.sender != env.contract.address && info.sender != owner.owner {
        return Err(ContractError::Unauthorized {});
    }

    let msg = transfer(deps, class_id, token_id, receiver)?;
    Ok(Response::new()
        .add_attribute("action", "transfer")
        .add_submessage(msg))
}

fn execute_burn(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
) -> Result<Response, ContractError> {
    // This will error if the class_id does not exist so no need to check
    let owner = get_owner(deps.as_ref(), class_id.clone(), token_id.clone())?;

    // Check if we are the owner or the contract itself
    if info.sender != env.contract.address && info.sender != owner.owner {
        return Err(ContractError::Unauthorized {});
    }

    let msg = burn(deps, class_id, token_id)?;
    Ok(Response::new()
        .add_attribute("action", "burn")
        .add_submessage(msg))
}

fn execute_mint(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_ids: Vec<String>,
    token_uris: Vec<String>,
    receiver: String,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    if token_ids.len() != token_uris.len() {
        return Err(ContractError::ImbalancedTokenInfo {});
    }
    let receiver = deps.api.addr_validate(&receiver)?;
    let cw721_addr = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;

    let mint_messages = token_ids
        .into_iter()
        // Can zip without worrying about dropping data as we assert
        // that lengths are the same above.
        .zip(token_uris.into_iter())
        .map(|(token_id, token_uri)| -> StdResult<WasmMsg> {
            let msg = cw721_base::msg::ExecuteMsg::Mint(cw721_base::MintMsg::<Empty> {
                token_id,
                token_uri: Some(token_uri),
                owner: receiver.to_string(),
                extension: Empty::default(),
            });
            Ok(WasmMsg::Execute {
                contract_addr: cw721_addr.to_string(),
                msg: to_binary(&msg)?,
                funds: vec![],
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(Response::default()
        .add_attribute("method", "execute_mint")
        .add_messages(mint_messages))
}

fn execute_do_instantiate_and_mint(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_ids: Vec<String>,
    token_uris: Vec<String>,
    receiver: String,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    // Optionally, instantiate a new cw721 contract if one does not
    // yet exist.
    let submessages = if CLASS_ID_TO_NFT_CONTRACT.has(deps.storage, class_id.clone()) {
        vec![]
    } else {
        let message = cw721_base::msg::InstantiateMsg {
            // Name of the collection MUST be class_id as this is how
            // we create a map entry on reply.
            name: class_id.clone(),
            symbol: class_id.clone(), // TODO: What should we put here?
            minter: env.contract.address.to_string(),
        };
        let message = WasmMsg::Instantiate {
            admin: None, // TODO: Any reason to set ourselves as admin?
            code_id: CW721_CODE_ID.load(deps.storage)?,
            msg: to_binary(&message)?,
            funds: vec![],
            label: format!("{} ICS721 cw721 backing contract", class_id),
        };
        let message = SubMsg::<Empty>::reply_always(message, INSTANTIATE_CW721_REPLY_ID);
        vec![message]
    };

    // Mint the requested tokens. Submessages and their replies are
    // always executed before regular messages [1], so we can sleep
    // nicely knowing this won't happen until the cw721 contract has
    // been instantiated.
    //
    // [1] https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#order-and-rollback
    let mint_message = WasmMsg::Execute {
        contract_addr: env.contract.address.into_string(),
        msg: to_binary(&ExecuteMsg::Mint {
            class_id,
            token_ids,
            token_uris,
            receiver,
        })?,
        funds: vec![],
    };

    Ok(Response::default()
        .add_attribute("method", "do_instantiate_and_mint")
        .add_submessages(submessages)
        .add_message(mint_message))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner { token_id, class_id } => {
            to_binary(&get_owner(deps, class_id, token_id)?)
        }
        QueryMsg::GetNft { class_id, token_id } => to_binary(&get_nft(deps, class_id, token_id)?),
        QueryMsg::HasClass { class_id } => to_binary(&has_class(deps, class_id)?),
        QueryMsg::GetClass { class_id } => to_binary(&get_class(deps, class_id)?),
    }
}
