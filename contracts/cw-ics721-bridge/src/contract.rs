#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{
    from_binary, to_binary, Addr, Binary, Deps, DepsMut, Empty, Env, IbcMsg, MessageInfo, Response,
    StdResult, SubMsg, WasmMsg,
};
use cw2::set_contract_version;

use crate::{
    error::ContractError,
    ibc::{NonFungibleTokenPacketData, INSTANTIATE_CW721_REPLY_ID, INSTANTIATE_PROXY_REPLY_ID},
    msg::{CallbackMsg, ExecuteMsg, IbcOutgoingMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    state::{
        UniversalNftInfoResponse, CLASS_ID_TO_CLASS, CLASS_ID_TO_NFT_CONTRACT,
        CLASS_TOKEN_ID_TO_TOKEN_METADATA, CW721_CODE_ID, NFT_CONTRACT_TO_CLASS,
        OUTGOING_CLASS_TOKEN_TO_CHANNEL, PO, PROXY,
    },
    token_types::{Class, ClassId, Token, TokenId, VoucherCreation, VoucherRedemption},
};

const CONTRACT_NAME: &str = "crates.io:cw-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    CW721_CODE_ID.save(deps.storage, &msg.cw721_base_code_id)?;
    PROXY.save(deps.storage, &None)?;
    PO.set_pauser(deps.storage, deps.api, msg.pauser.as_deref())?;

    let proxy_instantiate = msg
        .proxy
        .map(|m| m.into_wasm_msg(env.contract.address))
        .map(|wasm| SubMsg::reply_on_success(wasm, INSTANTIATE_PROXY_REPLY_ID))
        .map_or(vec![], |s| vec![s]);

    Ok(Response::default()
        .add_submessages(proxy_instantiate)
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
    PO.error_if_paused(deps.storage)?;
    match msg {
        ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
            sender,
            token_id,
            msg,
        }) => execute_receive_nft(deps, info, token_id, sender, msg),
        ExecuteMsg::ReceiveProxyNft { eyeball, msg } => {
            execute_receive_proxy_nft(deps, info, eyeball, msg)
        }
        ExecuteMsg::Pause {} => execute_pause(deps, info),
        ExecuteMsg::Callback(msg) => execute_callback(deps, env, info, msg),
    }
}

fn execute_callback(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: CallbackMsg,
) -> Result<Response, ContractError> {
    if info.sender != env.contract.address {
        Err(ContractError::Unauthorized {})
    } else {
        match msg {
            CallbackMsg::CreateVouchers { receiver, create } => {
                callback_create_vouchers(deps, env, receiver, create)
            }
            CallbackMsg::RedeemVouchers { receiver, redeem } => {
                callback_redeem_vouchers(deps, receiver, redeem)
            }
            CallbackMsg::Mint {
                class_id,
                tokens,
                receiver,
            } => callback_mint(deps, class_id, tokens, receiver),

            CallbackMsg::Conjunction { operands } => Ok(Response::default().add_messages(operands)),
        }
    }
}

fn execute_receive_proxy_nft(
    deps: DepsMut,
    info: MessageInfo,
    eyeball: String,
    msg: cw721::Cw721ReceiveMsg,
) -> Result<Response, ContractError> {
    if PROXY
        .load(deps.storage)?
        .map_or(true, |proxy| info.sender != proxy)
    {
        return Err(ContractError::Unauthorized {});
    }
    let mut info = info;
    info.sender = deps.api.addr_validate(&eyeball)?;
    let cw721::Cw721ReceiveMsg {
        token_id,
        sender,
        msg,
    } = msg;
    receive_nft(deps, info, TokenId::new(token_id), sender, msg)
}

fn execute_receive_nft(
    deps: DepsMut,
    info: MessageInfo,
    token_id: String,
    sender: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    if PROXY.load(deps.storage)?.is_some() {
        Err(ContractError::Unauthorized {})
    } else {
        receive_nft(deps, info, TokenId::new(token_id), sender, msg)
    }
}

fn execute_pause(deps: DepsMut, info: MessageInfo) -> Result<Response, ContractError> {
    PO.pause(deps.storage, &info.sender)?;
    Ok(Response::default().add_attribute("method", "pause"))
}

fn receive_nft(
    deps: DepsMut,
    info: MessageInfo,
    token_id: TokenId,
    sender: String,
    msg: Binary,
) -> Result<Response, ContractError> {
    let sender = deps.api.addr_validate(&sender)?;
    let msg: IbcOutgoingMsg = from_binary(&msg)?;
    let class = match NFT_CONTRACT_TO_CLASS.may_load(deps.storage, info.sender.clone())? {
        Some(class) => class,
        None => {
            // If we do not yet have a class ID for this contract, it is a
            // local NFT and its class ID is its conract address.

            // We set class level metadata and URI to None for local NFTs.
            let class = Class {
                id: ClassId::new(info.sender.to_string()),
                uri: None,
                data: None,
            };

            NFT_CONTRACT_TO_CLASS.save(deps.storage, info.sender.clone(), &class)?;
            CLASS_ID_TO_NFT_CONTRACT.save(deps.storage, class.id.clone(), &info.sender)?;

            // Merging and usage of this PR may change that:
            // <https://github.com/CosmWasm/cw-nfts/pull/75>
            CLASS_ID_TO_CLASS.save(deps.storage, class.id.clone(), &class)?;
            class
        }
    };

    let UniversalNftInfoResponse { token_uri, .. } = deps.querier.query_wasm_smart(
        info.sender,
        &cw721::Cw721QueryMsg::NftInfo {
            token_id: token_id.clone().into(),
        },
    )?;

    let ibc_message = NonFungibleTokenPacketData {
        class_id: class.id.clone(),
        class_uri: class.uri,
        class_data: class.data,

        token_ids: vec![token_id.clone()],
        token_uris: token_uri.map(|uri| vec![uri]),
        token_data: None, // Like class_uri and class_data, token_data is none for local NFTs.

        sender: sender.into_string(),
        receiver: msg.receiver,
        memo: msg.memo,
    };
    let ibc_message = IbcMsg::SendPacket {
        channel_id: msg.channel_id.clone(),
        data: to_binary(&ibc_message)?,
        timeout: msg.timeout,
    };

    OUTGOING_CLASS_TOKEN_TO_CHANNEL.save(
        deps.storage,
        (class.id.clone(), token_id.clone()),
        &msg.channel_id,
    )?;

    Ok(Response::default()
        .add_attribute("method", "execute_receive_nft")
        .add_attribute("token_id", token_id)
        .add_attribute("class_id", class.id)
        .add_attribute("channel_id", msg.channel_id)
        .add_message(ibc_message))
}

fn callback_mint(
    deps: DepsMut,
    class_id: ClassId,
    tokens: Vec<Token>,
    receiver: String,
) -> Result<Response, ContractError> {
    let receiver = deps.api.addr_validate(&receiver)?;
    let cw721_addr = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class_id.clone())?;

    let mint = tokens
        .into_iter()
        .map(|Token { id, uri, data }| {
            // We save token metadata here because it is part of the mint process
            CLASS_TOKEN_ID_TO_TOKEN_METADATA.save(
                deps.storage,
                (class_id.clone(), id.clone()),
                &data,
            )?;

            let msg = cw721_base::msg::ExecuteMsg::<Empty, Empty>::Mint(cw721_base::MintMsg {
                token_id: id.into(),
                token_uri: uri,
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
        .add_messages(mint))
}

fn callback_create_vouchers(
    deps: DepsMut,
    env: Env,
    receiver: String,
    create: VoucherCreation,
) -> Result<Response, ContractError> {
    let VoucherCreation { class, tokens } = create;
    let instantiate = if CLASS_ID_TO_NFT_CONTRACT.has(deps.storage, class.id.clone()) {
        vec![]
    } else {
        let message = SubMsg::<Empty>::reply_on_success(
            WasmMsg::Instantiate {
                admin: None,
                code_id: CW721_CODE_ID.load(deps.storage)?,
                msg: to_binary(&cw721_base::msg::InstantiateMsg {
                    // Name of the collection MUST be class_id as this is how
                    // we create a map entry on reply.
                    name: class.id.clone().into(),
                    symbol: class.id.clone().into(),
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
    CLASS_ID_TO_CLASS.save(deps.storage, class.id.clone(), &class)?;

    let mint = WasmMsg::Execute {
        contract_addr: env.contract.address.into_string(),
        msg: to_binary(&ExecuteMsg::Callback(CallbackMsg::Mint {
            class_id: class.id,
            receiver,
            tokens,
        }))?,
        funds: vec![],
    };

    Ok(Response::default()
        .add_attribute("method", "callback_create_vouchers")
        .add_submessages(instantiate)
        .add_message(mint))
}

fn callback_redeem_vouchers(
    deps: DepsMut,
    receiver: String,
    redeem: VoucherRedemption,
) -> Result<Response, ContractError> {
    let VoucherRedemption { class, token_ids } = redeem;
    let nft_contract = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, class.id.clone())?;
    let receiver = deps.api.addr_validate(&receiver)?;
    Ok(Response::default().add_messages(
        token_ids
            .into_iter()
            .map(|token_id| {
                // token metadata is set in mint, on voucher creation, and removed here on voucher redemption
                CLASS_TOKEN_ID_TO_TOKEN_METADATA
                    .remove(deps.storage, (class.id.clone(), token_id.clone()));

                Ok(WasmMsg::Execute {
                    contract_addr: nft_contract.to_string(),
                    msg: to_binary(&cw721::Cw721ExecuteMsg::TransferNft {
                        recipient: receiver.to_string(),
                        token_id: token_id.into(),
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
        QueryMsg::ClassId { contract } => to_binary(&query_class_for_nft_contract(deps, contract)?),
        QueryMsg::NftContract { class_id } => {
            to_binary(&query_nft_contract_for_class_id(deps, class_id)?)
        }
        QueryMsg::Metadata { class_id } => to_binary(&query_metadata(deps, class_id)?),
        QueryMsg::Owner { class_id, token_id } => {
            to_binary(&query_owner(deps, class_id, token_id)?)
        }
        QueryMsg::Pauser {} => to_binary(&PO.query_pauser(deps.storage)?),
        QueryMsg::Paused {} => to_binary(&PO.query_paused(deps.storage)?),
        QueryMsg::Proxy {} => to_binary(&PROXY.load(deps.storage)?),
    }
}

fn query_class_for_nft_contract(deps: Deps, contract: String) -> StdResult<Option<Class>> {
    let contract = deps.api.addr_validate(&contract)?;
    NFT_CONTRACT_TO_CLASS.may_load(deps.storage, contract)
}

fn query_nft_contract_for_class_id(deps: Deps, class_id: String) -> StdResult<Option<Addr>> {
    CLASS_ID_TO_NFT_CONTRACT.may_load(deps.storage, ClassId::new(class_id))
}

fn query_metadata(deps: Deps, class_id: String) -> StdResult<Option<Class>> {
    CLASS_ID_TO_CLASS.may_load(deps.storage, ClassId::new(class_id))
}

fn query_owner(
    deps: Deps,
    class_id: String,
    token_id: String,
) -> StdResult<cw721::OwnerOfResponse> {
    let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, ClassId::new(class_id))?;
    let resp: cw721::OwnerOfResponse = deps.querier.query_wasm_smart(
        class_uri,
        &cw721::Cw721QueryMsg::OwnerOf {
            token_id,
            include_expired: None,
        },
    )?;
    Ok(resp)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(deps: DepsMut, _env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    match msg {
        MigrateMsg::WithUpdate { pauser, proxy } => {
            PROXY.save(
                deps.storage,
                &proxy
                    .as_ref()
                    .map(|h| deps.api.addr_validate(h))
                    .transpose()?,
            )?;
            PO.set_pauser(deps.storage, deps.api, pauser.as_deref())?;
            Ok(Response::default().add_attribute("method", "migrate"))
        }
    }
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
        let token_id = TokenId::new("1");
        let sender = "ekez".to_string();
        let msg = to_binary(&IbcOutgoingMsg {
            receiver: "callum".to_string(),
            channel_id: "channel-1".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
            memo: None,
        })
        .unwrap();

        let res = receive_nft(deps.as_mut(), info, token_id.clone(), sender.clone(), msg).unwrap();
        assert_eq!(res.messages.len(), 1);

        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::Ibc(IbcMsg::SendPacket {
                channel_id: "channel-1".to_string(),
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
                data: to_binary(&NonFungibleTokenPacketData {
                    class_id: ClassId::new(NFT_ADDR),
                    class_uri: None,
                    class_data: None,
                    token_data: None,
                    token_ids: vec![token_id],
                    token_uris: Some(vec!["https://moonphase.is/image.svg".to_string()]),
                    sender,
                    receiver: "callum".to_string(),
                    memo: None,
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
        let token_id = TokenId::new("1");
        let sender = "ekez".to_string();
        let msg = to_binary(&IbcOutgoingMsg {
            receiver: "ekez".to_string(),
            channel_id: "channel-1".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_nanos(42)),
            memo: None,
        })
        .unwrap();

        receive_nft(deps.as_mut(), info, token_id, sender, msg).unwrap();

        let class = CLASS_ID_TO_CLASS
            .load(deps.as_ref().storage, ClassId::new(NFT_ADDR))
            .unwrap();
        assert_eq!(class.uri, None);
        assert_eq!(class.data, None);
    }
}
