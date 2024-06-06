use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json,
    testing::{mock_dependencies, mock_env, mock_info, MockQuerier, MOCK_CONTRACT_ADDR},
    to_json_binary, Addr, ContractResult, CosmosMsg, DepsMut, Empty, IbcMsg, IbcTimeout, Order,
    QuerierResult, Response, StdResult, SubMsg, Timestamp, WasmQuery,
};
use cw721::{
    msg::{AllNftInfoResponse, NftInfoResponse, NumTokensResponse},
    DefaultOptionalCollectionExtension, DefaultOptionalNftExtension,
};
use cw721_base::msg::QueryMsg;
use cw_cii::ContractInstantiateInfo;
use cw_ownable::Ownership;
use cw_storage_plus::Map;

use crate::{
    execute::Ics721Execute,
    ibc::{Ics721Ibc, INSTANTIATE_INCOMING_PROXY_REPLY_ID, INSTANTIATE_OUTGOING_PROXY_REPLY_ID},
    msg::{InstantiateMsg, MigrateMsg},
    query::{
        query_class_id_for_nft_contract, query_nft_contract_for_class_id, query_nft_contracts,
        Ics721Query,
    },
    state::{
        CollectionData, ADMIN_USED_FOR_CW721, CLASS_ID_TO_CLASS, CONTRACT_ADDR_LENGTH,
        CW721_CODE_ID, INCOMING_PROXY, OUTGOING_CLASS_TOKEN_TO_CHANNEL, OUTGOING_PROXY, PO,
    },
    utils::get_collection_data,
};
use ics721_types::{
    ibc_types::{IbcOutgoingMsg, NonFungibleTokenPacketData},
    token_types::{ClassId, TokenId},
};

const NFT_CONTRACT_1: &str = "nft1";
const NFT_CONTRACT_2: &str = "nft2";
const CLASS_ID_1: &str = "some/class/id1";
const CLASS_ID_2: &str = "some/class/id2";
const OWNER_ADDR: &str = "owner";
const ADMIN_ADDR: &str = "admin";
const PAUSER_ADDR: &str = "pauser";

#[derive(Default)]
pub struct Ics721Contract {}
impl Ics721Execute<Empty> for Ics721Contract {
    type ClassData = CollectionData;

    fn get_class_data(&self, deps: &DepsMut, sender: &Addr) -> StdResult<Option<Self::ClassData>> {
        get_collection_data(deps, sender).map(Option::Some)
    }
}
impl Ics721Ibc<Empty> for Ics721Contract {}
impl Ics721Query for Ics721Contract {}

#[derive(Default)]
pub struct Ics721ContractNoClassData {}
impl Ics721Execute<Empty> for Ics721ContractNoClassData {
    type ClassData = CollectionData;

    fn get_class_data(
        &self,
        _deps: &DepsMut,
        _sender: &Addr,
    ) -> StdResult<Option<Self::ClassData>> {
        Ok(None)
    }
}
impl Ics721Ibc<Empty> for Ics721ContractNoClassData {}
impl Ics721Query for Ics721ContractNoClassData {}

// copy of cosmwasm_std::ContractInfoResponse (marked as non-exhaustive)
#[cw_serde]
pub struct ContractInfoResponse {
    pub code_id: u64,
    /// address that instantiated this contract
    pub creator: String,
    /// admin who can run migrations (if any)
    pub admin: Option<String>,
    /// if set, the contract is pinned to the cache, and thus uses less gas when called
    pub pinned: bool,
    /// set if this contract has bound an IBC port
    pub ibc_port: Option<String>,
}

fn mock_querier(query: &WasmQuery) -> QuerierResult {
    match query {
        cosmwasm_std::WasmQuery::Smart {
            contract_addr: _,
            msg,
        } => match from_json::<
            cw721_base::msg::QueryMsg<
                DefaultOptionalNftExtension,
                DefaultOptionalCollectionExtension,
                Empty,
            >,
        >(&msg)
        .unwrap()
        {
            #[allow(deprecated)]
            QueryMsg::Minter {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&Ownership::<Addr> {
                    owner: Some(Addr::unchecked(OWNER_ADDR)),
                    pending_owner: None,
                    pending_expiry: None,
                })
                .unwrap(),
            )),
            #[allow(deprecated)]
            QueryMsg::Ownership {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&Ownership::<Addr> {
                    owner: Some(Addr::unchecked(OWNER_ADDR)),
                    pending_owner: None,
                    pending_expiry: None,
                })
                .unwrap(),
            )),
            QueryMsg::GetMinterOwnership {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&Ownership::<Addr> {
                    owner: Some(Addr::unchecked(OWNER_ADDR)),
                    pending_owner: None,
                    pending_expiry: None,
                })
                .unwrap(),
            )),
            QueryMsg::AllNftInfo { .. } => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&AllNftInfoResponse::<Option<Empty>> {
                    access: cw721::msg::OwnerOfResponse {
                        owner: MOCK_CONTRACT_ADDR.to_string(),
                        approvals: vec![],
                    },
                    info: NftInfoResponse {
                        token_uri: Some("https://moonphase.is/image.svg".to_string()),
                        extension: None,
                    },
                })
                .unwrap(),
            )),
            #[allow(deprecated)]
            QueryMsg::ContractInfo {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&cw721::msg::CollectionInfoAndExtensionResponse::<
                    DefaultOptionalCollectionExtension,
                > {
                    name: "name".to_string(),
                    symbol: "symbol".to_string(),
                    extension: DefaultOptionalCollectionExtension::default(),
                    updated_at: Timestamp::default(),
                })
                .unwrap(),
            )),
            QueryMsg::GetCollectionInfoAndExtension {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&cw721::msg::CollectionInfoAndExtensionResponse::<
                    DefaultOptionalCollectionExtension,
                > {
                    name: "name".to_string(),
                    symbol: "symbol".to_string(),
                    extension: DefaultOptionalCollectionExtension::default(),
                    updated_at: Timestamp::default(),
                })
                .unwrap(),
            )),
            QueryMsg::NumTokens {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&NumTokensResponse { count: 1 }).unwrap(),
            )),
            _ => unimplemented!(),
        },
        cosmwasm_std::WasmQuery::ContractInfo { .. } => QuerierResult::Ok(ContractResult::Ok(
            to_json_binary(&ContractInfoResponse {
                code_id: 0,
                creator: "creator".to_string(),
                admin: None,
                pinned: false,
                ibc_port: None,
            })
            .unwrap(),
        )),
        _ => unimplemented!(),
    }
}

fn mock_querier_v016(query: &WasmQuery) -> QuerierResult {
    match query {
        cosmwasm_std::WasmQuery::Smart {
            contract_addr: _,
            msg,
        } => match from_json::<cw721_base_016::msg::QueryMsg<Empty>>(&msg).unwrap() {
            // unwrap using latest (not old) cw721-base, since it is backwards compatible
            cw721_base_016::msg::QueryMsg::Minter {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(
                    // return v016 response
                    &cw721_base_016::msg::MinterResponse {
                        minter: OWNER_ADDR.to_string(),
                    },
                )
                .unwrap(),
            )),
            cw721_base_016::msg::QueryMsg::AllNftInfo { .. } => {
                QuerierResult::Ok(ContractResult::Ok(
                    to_json_binary(
                        // return v016 response
                        &cw721_016::AllNftInfoResponse::<Option<Empty>> {
                            access: cw721_016::OwnerOfResponse {
                                owner: MOCK_CONTRACT_ADDR.to_string(),
                                approvals: vec![],
                            },
                            info: cw721_016::NftInfoResponse {
                                token_uri: Some("https://moonphase.is/image.svg".to_string()),
                                extension: None,
                            },
                        },
                    )
                    .unwrap(),
                ))
            }
            cw721_base_016::msg::QueryMsg::ContractInfo {} => {
                QuerierResult::Ok(ContractResult::Ok(
                    to_json_binary(&cw721_016::ContractInfoResponse {
                        name: "name".to_string(),
                        symbol: "symbol".to_string(),
                    })
                    .unwrap(),
                ))
            }
            cw721_base_016::msg::QueryMsg::NumTokens {} => QuerierResult::Ok(ContractResult::Ok(
                to_json_binary(&cw721_016::NumTokensResponse { count: 1 }).unwrap(),
            )),
            _ => QuerierResult::Err(cosmwasm_std::SystemError::Unknown {}), // throws error for Ownership query
        },
        cosmwasm_std::WasmQuery::ContractInfo { .. } => QuerierResult::Ok(ContractResult::Ok(
            to_json_binary(&ContractInfoResponse {
                code_id: 0,
                creator: "creator".to_string(),
                admin: None,
                pinned: false,
                ibc_port: None,
            })
            .unwrap(),
        )),
        _ => unimplemented!(),
    }
}

#[test]
fn test_receive_nft() {
    // test case: receive nft from cw721-base
    let expected_contract_info: cosmwasm_std::ContractInfoResponse = from_json(
        to_json_binary(&ContractInfoResponse {
            code_id: 0,
            creator: "creator".to_string(),
            admin: None,
            pinned: false,
            ibc_port: None,
        })
        .unwrap(),
    )
    .unwrap();
    {
        let mut querier = MockQuerier::default();
        querier.update_wasm(mock_querier);

        let mut deps = mock_dependencies();
        deps.querier = querier;
        let env = mock_env();

        let info = mock_info(NFT_CONTRACT_1, &[]);
        let token_id = "1";
        let sender = "ekez".to_string();
        let msg = to_json_binary(&IbcOutgoingMsg {
            receiver: "callum".to_string(),
            channel_id: "channel-1".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
            memo: None,
        })
        .unwrap();

        let res: cosmwasm_std::Response<_> = Ics721Contract::default()
            .receive_nft(
                deps.as_mut(),
                env,
                &info.sender,
                TokenId::new(token_id),
                sender.clone(),
                msg,
            )
            .unwrap();
        assert_eq!(res.messages.len(), 1);

        let channel_id = "channel-1".to_string();
        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::<Empty>::Ibc(IbcMsg::SendPacket {
                channel_id: channel_id.clone(),
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
                data: to_json_binary(&NonFungibleTokenPacketData {
                    class_id: ClassId::new(NFT_CONTRACT_1),
                    class_uri: None,
                    class_data: Some(
                        to_json_binary(&CollectionData {
                            owner: Some(OWNER_ADDR.to_string()),
                            contract_info: Some(expected_contract_info.clone()),
                            name: "name".to_string(),
                            symbol: "symbol".to_string(),
                            num_tokens: Some(1),
                        })
                        .unwrap()
                    ),
                    token_data: None,
                    token_ids: vec![TokenId::new(token_id)],
                    token_uris: Some(vec!["https://moonphase.is/image.svg".to_string()]),
                    sender,
                    receiver: "callum".to_string(),
                    memo: None,
                })
                .unwrap()
            }))
        );

        // check outgoing classID and tokenID
        let keys = OUTGOING_CLASS_TOKEN_TO_CHANNEL
            .keys(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<(String, String)>>>()
            .unwrap();
        assert_eq!(keys, [(NFT_CONTRACT_1.to_string(), token_id.to_string())]);

        // check channel
        let key = (
            ClassId::new(keys[0].clone().0),
            TokenId::new(keys[0].clone().1),
        );
        assert_eq!(
            OUTGOING_CLASS_TOKEN_TO_CHANNEL
                .load(deps.as_mut().storage, key)
                .unwrap(),
            channel_id
        )
    }
    // test case: receive nft from old/v016 cw721-base
    {
        let mut querier = MockQuerier::default();
        querier.update_wasm(mock_querier_v016);

        let mut deps = mock_dependencies();
        deps.querier = querier;
        let env = mock_env();

        let info = mock_info(NFT_CONTRACT_1, &[]);
        let token_id = "1";
        let sender = "ekez".to_string();
        let msg = to_json_binary(&IbcOutgoingMsg {
            receiver: "callum".to_string(),
            channel_id: "channel-1".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
            memo: None,
        })
        .unwrap();

        let res: cosmwasm_std::Response<_> = Ics721Contract::default()
            .receive_nft(
                deps.as_mut(),
                env,
                &info.sender,
                TokenId::new(token_id),
                sender.clone(),
                msg,
            )
            .unwrap();
        assert_eq!(res.messages.len(), 1);

        let channel_id = "channel-1".to_string();
        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::<Empty>::Ibc(IbcMsg::SendPacket {
                channel_id: channel_id.clone(),
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
                data: to_json_binary(&NonFungibleTokenPacketData {
                    class_id: ClassId::new(NFT_CONTRACT_1),
                    class_uri: None,
                    class_data: Some(
                        to_json_binary(&CollectionData {
                            owner: Some(OWNER_ADDR.to_string()),
                            contract_info: Some(expected_contract_info),
                            name: "name".to_string(),
                            symbol: "symbol".to_string(),
                            num_tokens: Some(1),
                        })
                        .unwrap()
                    ),
                    token_data: None,
                    token_ids: vec![TokenId::new(token_id)],
                    token_uris: Some(vec!["https://moonphase.is/image.svg".to_string()]),
                    sender,
                    receiver: "callum".to_string(),
                    memo: None,
                })
                .unwrap()
            }))
        );

        // check outgoing classID and tokenID
        let keys = OUTGOING_CLASS_TOKEN_TO_CHANNEL
            .keys(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<(String, String)>>>()
            .unwrap();
        assert_eq!(keys, [(NFT_CONTRACT_1.to_string(), token_id.to_string())]);

        // check channel
        let key = (
            ClassId::new(keys[0].clone().0),
            TokenId::new(keys[0].clone().1),
        );
        assert_eq!(
            OUTGOING_CLASS_TOKEN_TO_CHANNEL
                .load(deps.as_mut().storage, key)
                .unwrap(),
            channel_id
        )
    }
    // test case: receive nft with no class data
    {
        let mut querier = MockQuerier::default();
        querier.update_wasm(mock_querier_v016);

        let mut deps = mock_dependencies();
        deps.querier = querier;
        let env = mock_env();

        let info = mock_info(NFT_CONTRACT_1, &[]);
        let token_id = "1";
        let sender = "ekez".to_string();
        let msg = to_json_binary(&IbcOutgoingMsg {
            receiver: "callum".to_string(),
            channel_id: "channel-1".to_string(),
            timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
            memo: None,
        })
        .unwrap();

        let res: cosmwasm_std::Response<_> = Ics721ContractNoClassData::default()
            .receive_nft(
                deps.as_mut(),
                env,
                &info.sender,
                TokenId::new(token_id),
                sender.clone(),
                msg,
            )
            .unwrap();
        assert_eq!(res.messages.len(), 1);

        let channel_id = "channel-1".to_string();
        assert_eq!(
            res.messages[0],
            SubMsg::new(CosmosMsg::<Empty>::Ibc(IbcMsg::SendPacket {
                channel_id: channel_id.clone(),
                timeout: IbcTimeout::with_timestamp(Timestamp::from_seconds(42)),
                data: to_json_binary(&NonFungibleTokenPacketData {
                    class_id: ClassId::new(NFT_CONTRACT_1),
                    class_uri: None,
                    class_data: None,
                    token_data: None,
                    token_ids: vec![TokenId::new(token_id)],
                    token_uris: Some(vec!["https://moonphase.is/image.svg".to_string()]),
                    sender,
                    receiver: "callum".to_string(),
                    memo: None,
                })
                .unwrap()
            }))
        );

        // check outgoing classID and tokenID
        let keys = OUTGOING_CLASS_TOKEN_TO_CHANNEL
            .keys(deps.as_mut().storage, None, None, Order::Ascending)
            .collect::<StdResult<Vec<(String, String)>>>()
            .unwrap();
        assert_eq!(keys, [(NFT_CONTRACT_1.to_string(), token_id.to_string())]);

        // check channel
        let key = (
            ClassId::new(keys[0].clone().0),
            TokenId::new(keys[0].clone().1),
        );
        assert_eq!(
            OUTGOING_CLASS_TOKEN_TO_CHANNEL
                .load(deps.as_mut().storage, key)
                .unwrap(),
            channel_id
        )
    }
}

#[test]
fn test_receive_sets_uri() {
    let mut querier = MockQuerier::default();
    querier.update_wasm(mock_querier);

    let mut deps = mock_dependencies();
    deps.querier = querier;
    let env = mock_env();

    let info = mock_info(NFT_CONTRACT_1, &[]);
    let token_id = TokenId::new("1");
    let sender = "ekez".to_string();
    let msg = to_json_binary(&IbcOutgoingMsg {
        receiver: "ekez".to_string(),
        channel_id: "channel-1".to_string(),
        timeout: IbcTimeout::with_timestamp(Timestamp::from_nanos(42)),
        memo: None,
    })
    .unwrap();

    Ics721Contract {}
        .receive_nft(deps.as_mut(), env, &info.sender, token_id, sender, msg)
        .unwrap();

    let class = CLASS_ID_TO_CLASS
        .load(deps.as_ref().storage, ClassId::new(NFT_CONTRACT_1))
        .unwrap();
    assert_eq!(class.uri, None);
    let expected_contract_info: cosmwasm_std::ContractInfoResponse = from_json(
        to_json_binary(&ContractInfoResponse {
            code_id: 0,
            creator: "creator".to_string(),
            admin: None,
            pinned: false,
            ibc_port: None,
        })
        .unwrap(),
    )
    .unwrap();
    assert_eq!(
        class.data,
        Some(
            to_json_binary(&CollectionData {
                owner: Some(OWNER_ADDR.to_string()),
                contract_info: Some(expected_contract_info),
                name: "name".to_string(),
                symbol: "symbol".to_string(),
                num_tokens: Some(1),
            })
            .unwrap()
        ),
    );
}

fn instantiate_msg(
    incoming_proxy: Option<ContractInstantiateInfo>,
    outgoing_proxy: Option<ContractInstantiateInfo>,
) -> InstantiateMsg {
    InstantiateMsg {
        cw721_base_code_id: 0,
        incoming_proxy,
        outgoing_proxy,
        pauser: Some(PAUSER_ADDR.to_string()),
        cw721_admin: Some(ADMIN_ADDR.to_string()),
        contract_addr_length: None,
    }
}

#[test]
fn test_instantiate() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info(OWNER_ADDR, &[]);
    let incoming_proxy_init_msg = ContractInstantiateInfo {
        code_id: 0,
        msg: to_json_binary("incoming").unwrap(),
        admin: Some(cw_cii::Admin::Address {
            addr: ADMIN_ADDR.to_string(),
        }),
        label: "incoming".to_string(),
    };
    let outgoing_proxy_init_msg = ContractInstantiateInfo {
        code_id: 0,
        msg: to_json_binary("outgoing").unwrap(),
        admin: Some(cw_cii::Admin::Address {
            addr: ADMIN_ADDR.to_string(),
        }),
        label: "outgoing".to_string(),
    };
    let mut msg = instantiate_msg(
        Some(incoming_proxy_init_msg.clone()),
        Some(outgoing_proxy_init_msg.clone()),
    );
    msg.contract_addr_length = Some(20);
    let response = Ics721Contract {}
        .instantiate(deps.as_mut(), env.clone(), info, msg.clone())
        .unwrap();

    let expected_incoming_proxy_msg =
        incoming_proxy_init_msg.into_wasm_msg(env.clone().contract.address);
    let expected_outgoing_proxy_msg = outgoing_proxy_init_msg.into_wasm_msg(env.contract.address);
    let expected_response = Response::<Empty>::default()
        .add_submessage(SubMsg::reply_on_success(
            expected_incoming_proxy_msg,
            INSTANTIATE_INCOMING_PROXY_REPLY_ID,
        ))
        .add_submessage(SubMsg::reply_on_success(
            expected_outgoing_proxy_msg,
            INSTANTIATE_OUTGOING_PROXY_REPLY_ID,
        ))
        .add_attribute("method", "instantiate")
        .add_attribute("cw721_code_id", msg.cw721_base_code_id.to_string())
        .add_attribute("cw721_admin", ADMIN_ADDR)
        .add_attribute("contract_addr_length", "20");
    assert_eq!(response, expected_response);
    assert_eq!(CW721_CODE_ID.load(&deps.storage).unwrap(), 0);
    // incoming and outgoing proxy initially set to None and set later in sub msg
    assert_eq!(OUTGOING_PROXY.load(&deps.storage).unwrap(), None);
    assert_eq!(INCOMING_PROXY.load(&deps.storage).unwrap(), None);
    assert_eq!(
        PO.pauser.load(&deps.storage).unwrap(),
        Some(Addr::unchecked(PAUSER_ADDR))
    );
    assert!(!PO.paused.load(&deps.storage).unwrap());
    assert_eq!(
        ADMIN_USED_FOR_CW721.load(&deps.storage).unwrap(),
        Some(Addr::unchecked(ADMIN_ADDR.to_string()))
    );
    assert_eq!(CONTRACT_ADDR_LENGTH.load(&deps.storage).unwrap(), 20);
}

#[test]
fn test_migrate() {
    let mut deps = mock_dependencies();
    let env = mock_env();
    let info = mock_info(OWNER_ADDR, &[]);
    let msg = instantiate_msg(None, None);
    Ics721Contract {}
        .instantiate(deps.as_mut(), env.clone(), info, msg.clone())
        .unwrap();
    let msg = MigrateMsg::WithUpdate {
        pauser: Some("some_other_pauser".to_string()),
        outgoing_proxy: Some("outgoing".to_string()),
        incoming_proxy: Some("incoming".to_string()),
        cw721_base_code_id: Some(1),
        cw721_admin: Some("some_other_admin".to_string()),
        contract_addr_length: Some(20),
    };

    // before migrate, populate legacy
    let class_id_to_nft_contract: Map<ClassId, Addr> = Map::new("e");
    class_id_to_nft_contract
        .save(
            deps.as_mut().storage,
            ClassId::new(CLASS_ID_1),
            &Addr::unchecked(NFT_CONTRACT_1),
        )
        .unwrap();
    class_id_to_nft_contract
        .save(
            deps.as_mut().storage,
            ClassId::new(CLASS_ID_2),
            &Addr::unchecked(NFT_CONTRACT_2),
        )
        .unwrap();
    let nft_contract_to_class_id: Map<Addr, ClassId> = Map::new("f");
    nft_contract_to_class_id
        .save(
            deps.as_mut().storage,
            Addr::unchecked(NFT_CONTRACT_1),
            &ClassId::new(CLASS_ID_1),
        )
        .unwrap();
    nft_contract_to_class_id
        .save(
            deps.as_mut().storage,
            Addr::unchecked(NFT_CONTRACT_2),
            &ClassId::new(CLASS_ID_2),
        )
        .unwrap();

    // migrate
    Ics721Contract {}
        .migrate(deps.as_mut(), env.clone(), msg)
        .unwrap();

    assert_eq!(
        PO.pauser.load(&deps.storage).unwrap(),
        Some(Addr::unchecked("some_other_pauser"))
    );
    assert_eq!(
        OUTGOING_PROXY.load(&deps.storage).unwrap(),
        Some(Addr::unchecked("outgoing"))
    );
    assert_eq!(
        INCOMING_PROXY.load(&deps.storage).unwrap(),
        Some(Addr::unchecked("incoming"))
    );
    assert_eq!(CW721_CODE_ID.load(&deps.storage).unwrap(), 1);
    assert_eq!(
        ADMIN_USED_FOR_CW721.load(&deps.storage).unwrap(),
        Some(Addr::unchecked("some_other_admin"))
    );
    assert_eq!(CONTRACT_ADDR_LENGTH.load(&deps.storage).unwrap(), 20);
    let nft_contract_and_class_id_list = query_nft_contracts(deps.as_ref(), None, None).unwrap();
    assert_eq!(nft_contract_and_class_id_list.len(), 2);
    assert_eq!(nft_contract_and_class_id_list[0].0, CLASS_ID_1);
    assert_eq!(nft_contract_and_class_id_list[0].1, NFT_CONTRACT_1);
    assert_eq!(nft_contract_and_class_id_list[1].0, CLASS_ID_2);
    assert_eq!(nft_contract_and_class_id_list[1].1, NFT_CONTRACT_2);
    // test query and indexers for class id and addr are working
    let nft_contract_1 =
        query_nft_contract_for_class_id(&deps.storage, CLASS_ID_1.to_string().into()).unwrap();
    assert_eq!(nft_contract_1, Some(Addr::unchecked(NFT_CONTRACT_1)));
    let nft_contract_2 =
        query_nft_contract_for_class_id(&deps.storage, CLASS_ID_2.to_string().into()).unwrap();
    assert_eq!(nft_contract_2, Some(Addr::unchecked(NFT_CONTRACT_2)));
    let class_id_1 =
        query_class_id_for_nft_contract(deps.as_ref(), NFT_CONTRACT_1.to_string()).unwrap();
    assert_eq!(class_id_1, Some(ClassId::new(CLASS_ID_1)));
    let class_id_2 =
        query_class_id_for_nft_contract(deps.as_ref(), NFT_CONTRACT_2.to_string()).unwrap();
    assert_eq!(class_id_2, Some(ClassId::new(CLASS_ID_2)));
}
