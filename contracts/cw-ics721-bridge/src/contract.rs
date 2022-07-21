#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Empty, Env, IbcMsg, MessageInfo, Response,
    StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;

use crate::error::ContractError;
use crate::helpers::{
    burn, get_class, get_nft, get_owner, has_class, transfer, INSTANTIATE_CW721_REPLY_ID,
};
use crate::ibc::NonFungibleTokenPacketData;
use crate::msg::{ExecuteMsg, IbcAwayMsg, InstantiateMsg, QueryMsg};
use crate::state::{
    UniversalNftInfoResponse, CHANNELS, CLASS_ID_TO_CLASS_URI, CLASS_ID_TO_NFT_CONTRACT,
    CW721_CODE_ID, NFT_CONTRACT_TO_CLASS_ID,
};

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
        } => execute_transfer(deps.as_ref(), env, info, class_id, token_id, receiver),
        ExecuteMsg::Burn { class_id, token_id } => {
            execute_burn(deps.as_ref(), env, info, class_id, token_id)
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
            class_uri,
            token_ids,
            token_uris,
            receiver,
        } => execute_do_instantiate_and_mint(
            deps, env, info, class_id, class_uri, token_ids, token_uris, receiver,
        ),
        ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
            sender,
            token_id,
            msg,
        }) => execute_receive_nft(deps, info, token_id, sender, msg),
        ExecuteMsg::BatchTransferFromChannel {
            channel,
            class_id,
            token_ids,
            receiver,
        } => execute_batch_transfer_from_channel(
            deps.as_ref(),
            info,
            env,
            channel,
            class_id,
            token_ids,
            receiver,
        ),
        ExecuteMsg::BurnEscrowTokens {
            channel,
            class_id,
            token_ids,
        } => execute_burn_escrow_tokens(deps.as_ref(), env, info, channel, class_id, token_ids),
    }
}

fn execute_transfer(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
    receiver: String,
) -> Result<Response, ContractError> {
    // This will error if the class_id does not exist so no need to check
    let owner = get_owner(deps, class_id.clone(), token_id.clone())?;

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
    deps: Deps,
    env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
) -> Result<Response, ContractError> {
    // This will error if the class_id does not exist so no need to check
    let owner = get_owner(deps, class_id.clone(), token_id.clone())?;

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

#[allow(clippy::too_many_arguments)]
fn execute_do_instantiate_and_mint(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    class_id: String,
    class_uri: Option<String>,
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
        let expected = CLASS_ID_TO_CLASS_URI.load(deps.storage, class_id.clone())?;
        if expected != class_uri {
            // classUri information for a classID shouldn't change
            // across sends. TODO: is this the case? what are we
            // suposed to do if not..?
            return Err(ContractError::ClassUriClash {
                class_id,
                expected,
                actual: class_uri,
            });
        }
        vec![]
    } else {
        // Store mapping from classID to classUri. cw721 does not do
        // this, so we need to do it to stop the infomation from
        // getting lost.
        CLASS_ID_TO_CLASS_URI.save(deps.storage, class_id.clone(), &class_uri)?;

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
        let message = SubMsg::<Empty>::reply_on_success(message, INSTANTIATE_CW721_REPLY_ID);
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

fn execute_receive_nft(
    deps: DepsMut,
    info: MessageInfo,
    token_id: String,
    sender: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(&sender)?;
    let msg: IbcAwayMsg = from_binary(&msg)?;

    // When we receive a NFT over IBC we want to avoid parsing of the
    // classId field at all costs. String parsing has interesting time
    // complexity issues and is error prone.
    //
    // We would normally need to parse the classId field when we
    // receive a NFT from another chain, and the top of the classId
    // stack is our own chain. In this case, it is possible that upon
    // that NFT arriving at this chain it will be unpacked back into a
    // native NFT. To determine this we could either:
    //
    // (1) Check if the classId has no '/' characters. Or,
    // (2) Store each NFT we receive in our maps (giving it a classID
    //     of its own address) and do the same lookup / transfer
    //     regardless of if the NFT is becoming a native one.
    //
    // To avoid special logic when receiving packets, we go the save
    // route. We can change this back later if we'd like.
    if !NFT_CONTRACT_TO_CLASS_ID.has(deps.storage, info.sender.clone()) {
        // This is a new NFT so we give it a "classID". If the map
        // does have the key, it means that either (a) we have seen
        // the contract before, or (b) this is a NFT that we received
        // over IBC and made a new contract for.
        NFT_CONTRACT_TO_CLASS_ID.save(
            deps.storage,
            info.sender.clone(),
            &info.sender.to_string(),
        )?;
        CLASS_ID_TO_NFT_CONTRACT.save(deps.storage, info.sender.to_string(), &info.sender)?;
    }

    // Class ID is the IBCd ID, or the contract address.
    let class_id = NFT_CONTRACT_TO_CLASS_ID.load(deps.storage, info.sender.clone())?;

    // Can't allow specifying the class URI in the message as this
    // could cause multiple different class IDs to be submitted for
    // the same NFT collection by users. No way to decide on 'correct'
    // one.
    let class_uri = CLASS_ID_TO_CLASS_URI
        .may_load(deps.storage, class_id.clone())?
        .flatten();

    let UniversalNftInfoResponse { token_uri, .. } = deps.querier.query_wasm_smart(
        info.sender.clone(),
        &cw721::Cw721QueryMsg::NftInfo {
            token_id: token_id.clone(),
        },
    )?;

    let ibc_message = NonFungibleTokenPacketData {
        classId: class_id.clone(),
        classUri: class_uri,
        tokenIds: vec![token_id.clone()],
        tokenUris: vec![token_uri.unwrap_or_default()], // Think about this later..
        sender: sender.into_string(),
        receiver: msg.receiver,
    };
    let ibc_message = IbcMsg::SendPacket {
        channel_id: msg.channel_id.clone(),
        data: to_binary(&ibc_message)?,
        timeout: msg.timeout,
    };

    // Transfer message to send NFT to escrow for channel.
    let channel_escrow = CHANNELS.load(deps.storage, msg.channel_id.clone())?;

    let transfer_message = cw721::Cw721ExecuteMsg::TransferNft {
        recipient: channel_escrow.to_string(),
        token_id: token_id.clone(),
    };
    let transfer_message = WasmMsg::Execute {
        contract_addr: info.sender.into_string(),
        msg: to_binary(&transfer_message)?,
        funds: vec![],
    };

    Ok(Response::default()
        .add_attribute("method", "execute_receive_nft")
        .add_attribute("token_id", token_id)
        .add_attribute("class_id", class_id)
        .add_attribute("escrow", channel_escrow)
        .add_attribute("channel_id", msg.channel_id)
        .add_message(transfer_message)
        .add_message(ibc_message))
}

fn execute_batch_transfer_from_channel(
    deps: Deps,
    info: MessageInfo,
    env: Env,
    channel: String,
    class_id: String,
    token_ids: Vec<String>,
    receiver: String,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    let escrow_addr = CHANNELS.load(deps.storage, channel.clone())?;
    let cw721_addr = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;

    let transfer_messages = token_ids
        .iter()
        .map(|token_id| -> StdResult<WasmMsg> {
            let message = ics_escrow::msg::ExecuteMsg::Withdraw {
                nft_address: cw721_addr.to_string(),
                token_id: token_id.to_string(),
                receiver: receiver.clone(),
            };
            Ok(WasmMsg::Execute {
                contract_addr: escrow_addr.to_string(),
                msg: to_binary(&message)?,
                funds: vec![],
            })
        })
        .collect::<StdResult<Vec<_>>>()?;

    Ok(Response::default()
        .add_attribute("method", "execute_batch_transfer_from_channel")
        .add_attribute("channel", channel)
        .add_attribute("token_ids", format!("{:?}", token_ids))
        .add_attribute("receiver", receiver)
        .add_messages(transfer_messages))
}

fn execute_burn_escrow_tokens(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    channel: String,
    class_id: String,
    token_ids: Vec<String>,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }

    let escrow_addr = CHANNELS.load(deps.storage, channel.clone())?;
    let cw721_addr = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id.clone())?;

    Ok(Response::default()
        .add_attribute("method", "burn_escrow_tokens")
        .add_attribute("class_id", class_id)
        .add_attribute("channel", channel)
        .add_attribute("token_ids", format!("{:?}", token_ids))
        .add_message(WasmMsg::Execute {
            contract_addr: escrow_addr.into_string(),
            msg: to_binary(&ics_escrow::msg::ExecuteMsg::Burn {
                nft_address: cw721_addr.into_string(),
                token_ids,
            })?,
            funds: vec![],
        }))
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
