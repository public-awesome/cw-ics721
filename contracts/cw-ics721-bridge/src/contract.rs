#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Empty, Env, IbcMsg, MessageInfo, Response,
    StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;

use crate::{
    error::ContractError,
    ibc::{NonFungibleTokenPacketData, INSTANTIATE_CW721_REPLY_ID},
    msg::{
        CallbackMsg, ExecuteMsg, IbcAwayMsg, InstantiateMsg, NewTokenInfo, QueryMsg, TransferInfo,
    },
    state::{
        UniversalNftInfoResponse, CLASS_ID_TO_CLASS_URI, CLASS_ID_TO_NFT_CONTRACT,
        CW721_ICS_CODE_ID, NFT_CONTRACT_TO_CLASS_ID, OUTGOING_CLASS_TOKEN_TO_CHANNEL,
    },
};

const CONTRACT_NAME: &str = "crates.io:cw-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CW721_ICS_CODE_ID.save(deps.storage, &msg.cw721_base_code_id)?;

    Ok(Response::default()
        .add_attribute("method", "instantiate")
        .add_attribute("cw721_code_id", msg.cw721_base_code_id.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
            sender,
            token_id,
            msg,
        }) => execute_receive_nft(deps, info, token_id, sender, msg),

        ExecuteMsg::Callback(CallbackMsg::Mint {
            class_id,
            token_ids,
            token_uris,
            receiver,
        }) => execute_mint(
            deps.as_ref(),
            env,
            info,
            class_id,
            token_ids,
            token_uris,
            receiver,
        ),
        ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
            class_id,
            class_uri,
            token_ids,
            token_uris,
            receiver,
        }) => execute_do_instantiate_and_mint(
            deps, env, info, class_id, class_uri, token_ids, token_uris, receiver,
        ),
        ExecuteMsg::Callback(CallbackMsg::BatchTransfer {
            class_id,
            receiver,
            token_ids,
        }) => execute_batch_transfer(deps.as_ref(), env, info, class_id, receiver, token_ids),
        ExecuteMsg::Callback(CallbackMsg::HandlePacketReceive {
            receiver,
            class_uri,
            transfers,
            new_tokens,
        }) => execute_handle_packet_receive(
            deps.as_ref(),
            env,
            info,
            receiver,
            class_uri,
            transfers,
            new_tokens,
        ),
    }
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

    let class_id = if NFT_CONTRACT_TO_CLASS_ID.has(deps.storage, info.sender.clone()) {
        NFT_CONTRACT_TO_CLASS_ID.load(deps.storage, info.sender.clone())?
    } else {
        let class_id = info.sender.to_string();
        // If we do not yet have a class ID for this contract, it is a
        // local NFT and its class ID is its conract address.
        NFT_CONTRACT_TO_CLASS_ID.save(deps.storage, info.sender.clone(), &class_id)?;
        CLASS_ID_TO_NFT_CONTRACT.save(deps.storage, info.sender.to_string(), &info.sender)?;
        // We set class level metadata to None for local NFTs.
        //
        // Merging and usage of this PR may change that:
        // <https://github.com/CosmWasm/cw-nfts/pull/75>
        CLASS_ID_TO_CLASS_URI.save(deps.storage, info.sender.to_string(), &None)?;
        class_id
    };

    let class_uri = CLASS_ID_TO_CLASS_URI
        .may_load(deps.storage, class_id.clone())?
        .flatten();

    let UniversalNftInfoResponse { token_uri, .. } = deps.querier.query_wasm_smart(
        info.sender,
        &cw721::Cw721QueryMsg::NftInfo {
            token_id: token_id.clone(),
        },
    )?;

    let ibc_message = NonFungibleTokenPacketData {
        class_id: class_id.clone(),
        class_uri,
        token_ids: vec![token_id.clone()],
        token_uris: vec![token_uri.unwrap_or_default()], /* Currently token_uri is optional in
                                                          * cw721 - we set to empty string as
                                                          * default. */
        sender: sender.into_string(),
        receiver: msg.receiver,
    };
    let ibc_message = IbcMsg::SendPacket {
        channel_id: msg.channel_id.clone(),
        data: to_binary(&ibc_message)?,
        timeout: msg.timeout,
    };

    OUTGOING_CLASS_TOKEN_TO_CHANNEL.save(
        deps.storage,
        (class_id.clone(), token_id.clone()),
        &msg.channel_id,
    )?;

    Ok(Response::default()
        .add_attribute("method", "execute_receive_nft")
        .add_attribute("token_id", token_id)
        .add_attribute("class_id", class_id)
        .add_attribute("channel_id", msg.channel_id)
        .add_message(ibc_message))
}

fn execute_handle_packet_receive(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    receiver: String,
    class_uri: Option<String>,
    transfers: Option<TransferInfo>,
    new_tokens: Option<NewTokenInfo>,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    let receiver = deps.api.addr_validate(&receiver)?;

    let mut messages = Vec::with_capacity(2);
    if let Some(transfer_info) = transfers {
        messages.push(transfer_info.into_wasm_msg(&env, &receiver)?)
    }
    if let Some(token_info) = new_tokens {
        messages.push(token_info.into_wasm_msg(&env, &receiver, class_uri)?)
    }

    Ok(Response::default()
        .add_attribute("method", "handle_packet_receive")
        .add_messages(messages))
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
        // NOTE: We do not check that the incoming `class_uri` matches
        // the `class_uri` of other NFTs we have seen that are part of
        // the collection. The result of this is that, from the
        // perspective of our chain, the first NFT of a collection to
        // arrive sets the class level metadata for all preceding
        // NFTs.
        vec![]
    } else {
        let message = SubMsg::<Empty>::reply_on_success(
            WasmMsg::Instantiate {
                admin: None, // TODO: Any reason to set ourselves as admin?
                code_id: CW721_ICS_CODE_ID.load(deps.storage)?,
                msg: to_binary(&cw721_base::msg::InstantiateMsg {
                    // Name of the collection MUST be class_id as this is how
                    // we create a map entry on reply.
                    name: class_id.to_string(),
                    symbol: class_id.to_string(), // TODO: What should we put here?
                    minter: env.contract.address.to_string(),
                })?,
                funds: vec![],
                // Attempting to fit the class ID in the label field
                // can make this field too long which causes weird
                // data errors in the SDK.
                label: "ICS771 backing CW721".to_string(),
            },
            INSTANTIATE_CW721_REPLY_ID,
        );
        vec![message]
    };

    // Store mapping from classID to classURI. Notably, we don't check
    // if this has already been set. If a new NFT belonging to a class
    // ID we have already seen comes in with new metadata, we assume
    // that the metadata has been updated on the source chain and
    // update it for the class ID locally as well.
    CLASS_ID_TO_CLASS_URI.save(deps.storage, class_id.clone(), &class_uri)?;

    // Mint the requested tokens. Submessages and their replies are
    // always executed before regular messages [1], so we can sleep
    // nicely knowing this won't happen until the cw721 contract has
    // been instantiated.
    //
    // [1] https://github.com/CosmWasm/cosmwasm/blob/main/SEMANTICS.md#order-and-rollback
    let mint_message = WasmMsg::Execute {
        contract_addr: env.contract.address.into_string(),
        msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Mint {
            class_id,
            token_ids,
            token_uris,
            receiver,
        }))?,
        funds: vec![],
    };

    Ok(Response::default()
        .add_attribute("method", "do_instantiate_and_mint")
        .add_submessages(submessages)
        .add_message(mint_message))
}

fn execute_batch_transfer(
    deps: Deps,
    env: Env,
    info: MessageInfo,
    class_id: String,
    receiver: String,
    token_ids: Vec<String>,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        return Err(ContractError::Unauthorized {});
    }
    let nft_contract = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;
    let receiver = deps.api.addr_validate(&receiver)?;
    Ok(Response::default().add_messages(
        token_ids
            .into_iter()
            .map(|token_id| {
                Ok(WasmMsg::Execute {
                    contract_addr: nft_contract.to_string(),
                    msg: to_binary(&cw721::Cw721ExecuteMsg::TransferNft {
                        recipient: receiver.to_string(),
                        token_id,
                    })?,
                    funds: vec![],
                })
            })
            .collect::<StdResult<Vec<WasmMsg>>>()?,
    ))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::ClassIdForNftContract { contract } => {
            to_binary(&query_class_id_for_nft_contract(deps, contract)?)
        }
        QueryMsg::NftContractForClassId { class_id } => {
            to_binary(&query_nft_contract_for_class_id(deps, class_id)?)
        }
        QueryMsg::Metadata { class_id } => to_binary(&query_metadata(deps, class_id)?),
        QueryMsg::Owner { class_id, token_id } => {
            to_binary(&query_owner(deps, class_id, token_id)?)
        }
    }
}

pub fn query_class_id_for_nft_contract(deps: Deps, contract: String) -> StdResult<Option<String>> {
    let contract = deps.api.addr_validate(&contract)?;
    NFT_CONTRACT_TO_CLASS_ID.may_load(deps.storage, contract)
}

pub fn query_nft_contract_for_class_id(deps: Deps, class_id: String) -> StdResult<Option<Addr>> {
    CLASS_ID_TO_NFT_CONTRACT.may_load(deps.storage, class_id)
}

pub fn query_metadata(deps: Deps, class_id: String) -> StdResult<Option<String>> {
    Ok(CLASS_ID_TO_CLASS_URI
        .may_load(deps.storage, class_id)?
        .flatten())
}

pub fn query_owner(
    deps: Deps,
    class_id: String,
    token_id: String,
) -> StdResult<cw721::OwnerOfResponse> {
    let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id)?;
    let resp: cw721::OwnerOfResponse = deps.querier.query_wasm_smart(
        class_uri,
        &cw721::Cw721QueryMsg::OwnerOf {
            token_id,
            include_expired: None,
        },
    )?;
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_info, MockQuerier},
        ContractResult, CosmosMsg, IbcTimeout, QuerierResult, Timestamp, WasmQuery,
    };
    use cw721::NftInfoResponse;

    use super::*;

    const NFT_ADDR: &str = "nft";

    fn nft_info_response_mock_querier(query: &WasmQuery) -> QuerierResult {
        match query {
            cosmwasm_std::WasmQuery::Smart {
                contract_addr,
                msg: _,
            } => {
                if *contract_addr == NFT_ADDR {
                    QuerierResult::Ok(ContractResult::Ok(
                        to_binary(&NftInfoResponse::<Option<Empty>> {
                            token_uri: Some("https://moonphase.is/image.svg".to_string()),
                            extension: None,
                        })
                        .unwrap(),
                    ))
                } else {
                    unimplemented!()
                }
            }
            cosmwasm_std::WasmQuery::Raw {
                contract_addr: _,
                key: _,
            } => unimplemented!(),
            cosmwasm_std::WasmQuery::ContractInfo { contract_addr: _ } => unimplemented!(),
            _ => unimplemented!(),
        }
    }

    #[test]
    fn test_receive_nft() {
        let mut querier = MockQuerier::default();
        querier.update_wasm(nft_info_response_mock_querier);

        let mut deps = mock_dependencies();
        deps.querier = querier;

        let info = mock_info(NFT_ADDR, &[]);
        let token_id = "1".to_string();
        let sender = "ekez".to_string();
        let msg = to_binary(&IbcAwayMsg {
            receiver: "callum".to_string(),
            channel_id: "channel-1".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
        })
        .unwrap();

        let res = execute_receive_nft(deps.as_mut(), info, token_id, sender, msg).unwrap();
        assert_eq!(res.messages.len(), 1);

        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::Ibc(IbcMsg::SendPacket {
                channel_id: "channel-1".to_string(),
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
                data: to_binary(&NonFungibleTokenPacketData {
                    class_id: NFT_ADDR.to_string(),
                    class_uri: None,
                    token_ids: vec!["1".to_string()],
                    token_uris: vec!["https://moonphase.is/image.svg".to_string()],
                    sender: "ekez".to_string(),
                    receiver: "callum".to_string(),
                })
                .unwrap()
            }))
        )
    }

    #[test]
    fn test_receive_sets_uri() {
        let mut querier = MockQuerier::default();
        querier.update_wasm(nft_info_response_mock_querier);

        let mut deps = mock_dependencies();
        deps.querier = querier;

        let info = mock_info(NFT_ADDR, &[]);
        let token_id = "1".to_string();
        let sender = "ekez".to_string();
        let msg = to_binary(&IbcAwayMsg {
            receiver: "ekez".to_string(),
            channel_id: "channel-1".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_nanos(42)),
        })
        .unwrap();

        execute_receive_nft(deps.as_mut(), info, token_id, sender, msg).unwrap();

        let class_uri = CLASS_ID_TO_CLASS_URI
            .load(deps.as_ref().storage, "nft".to_string())
            .unwrap();
        assert_eq!(class_uri, None);
    }
}
