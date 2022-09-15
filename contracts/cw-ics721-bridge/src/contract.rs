#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Binary, Deps, DepsMut, Empty, Env, IbcMsg, MessageInfo, Response,
    StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;

use crate::{
    error::ContractError,
    helpers::{
        get_class, get_class_id_for_nft_contract, get_nft, get_owner, get_uri, has_class,
        list_class_ids, INSTANTIATE_CW721_REPLY_ID,
    },
    ibc::NonFungibleTokenPacketData,
    msg::{CallbackMsg, ExecuteMsg, IbcAwayMsg, InstantiateMsg, QueryMsg},
    state::{
        UniversalNftInfoResponse, CLASS_ID_TO_CLASS_URI, CLASS_ID_TO_NFT_CONTRACT,
        CW721_ICS_CODE_ID, ESCROW_CODE_ID, NFT_CONTRACT_TO_CLASS_ID,
        OUTGOING_CLASS_TOKEN_TO_CHANNEL,
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
    ESCROW_CODE_ID.save(deps.storage, &msg.escrow_code_id)?;

    Ok(Response::default()
        .add_attribute("method", "instantiate")
        .add_attribute("cw721_code_id", msg.cw721_base_code_id.to_string())
        .add_attribute("escrow_code_id", msg.escrow_code_id.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
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
        }) => execute_batch_transfer(deps.as_ref(), class_id, receiver, token_ids),
        ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
            sender,
            token_id,
            msg,
        }) => execute_receive_nft(deps, info, token_id, sender, msg),
    }
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
        // Store mapping from classID to classUri. cw721 does not do
        // this, so we need to do it to stop the infomation from
        // getting lost.
        CLASS_ID_TO_CLASS_URI.save(deps.storage, class_id.clone(), &class_uri)?;

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
                label: "ICS771 backing cw721".to_string(),
            },
            INSTANTIATE_CW721_REPLY_ID,
        );
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
    class_id: String,
    receiver: String,
    token_ids: Vec<String>,
) -> Result<Response, ContractError> {
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
            .collect::<StdResult<Vec<WasmMsg>>>()?
            .into_iter(),
    ))
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
        CLASS_ID_TO_CLASS_URI.save(deps.storage, info.sender.to_string(), &None)?;
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

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetOwner { token_id, class_id } => {
            to_binary(&get_owner(deps, class_id, token_id)?)
        }
        QueryMsg::GetNft { class_id, token_id } => to_binary(&get_nft(deps, class_id, token_id)?),
        QueryMsg::HasClass { class_id } => to_binary(&has_class(deps, class_id)?),
        QueryMsg::GetClass { class_id } => to_binary(&get_class(deps, class_id)?),
        QueryMsg::GetUri { class_id } => to_binary(&get_uri(deps, class_id)?),
        QueryMsg::ListClassIds { start_after, limit } => {
            to_binary(&list_class_ids(deps, start_after, limit)?)
        }
        QueryMsg::GetClassIdForNftContract { contract } => {
            to_binary(&get_class_id_for_nft_contract(deps, contract)?)
        }
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_info, MockQuerier},
        Addr, ContractResult, CosmosMsg, IbcTimeout, QuerierResult, Timestamp, WasmQuery,
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

        CHANNELS
            .save(
                deps.as_mut().storage,
                "channel-1".to_string(),
                &Addr::unchecked("escrow"),
            )
            .unwrap();

        let res = execute_receive_nft(deps.as_mut(), info, token_id, sender, msg).unwrap();
        assert_eq!(res.messages.len(), 2);

        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::Wasm(WasmMsg::Execute {
                contract_addr: "nft".to_string(),
                funds: vec![],
                msg: to_binary(&cw721::Cw721ExecuteMsg::TransferNft {
                    recipient: "escrow".to_string(),
                    token_id: "1".to_string(),
                })
                .unwrap()
            }))
        );

        assert_eq!(
            res.messages[1],
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

        CHANNELS
            .save(
                deps.as_mut().storage,
                "channel-1".to_string(),
                &Addr::unchecked("escrow"),
            )
            .unwrap();

        execute_receive_nft(deps.as_mut(), info, token_id, sender, msg).unwrap();

        let class_uri = CLASS_ID_TO_CLASS_URI
            .load(deps.as_ref().storage, "nft".to_string())
            .unwrap();
        assert_eq!(class_uri, None);
    }
}
