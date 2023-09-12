use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_binary,
    testing::{mock_dependencies, mock_env, mock_info, MockQuerier, MOCK_CONTRACT_ADDR},
    to_binary, Addr, ContractResult, CosmosMsg, DepsMut, Empty, IbcMsg, IbcTimeout, Order,
    QuerierResult, StdResult, SubMsg, Timestamp, WasmQuery,
};
use cw721::{AllNftInfoResponse, NftInfoResponse, NumTokensResponse};
use cw721_base::QueryMsg;
use cw_ownable::Ownership;

use crate::{
    execute::Ics721Execute,
    ibc::{Ics721Ibc, NonFungibleTokenPacketData},
    msg::IbcOutgoingMsg,
    query::Ics721Query,
    state::{CollectionData, CLASS_ID_TO_CLASS, OUTGOING_CLASS_TOKEN_TO_CHANNEL},
    token_types::{ClassId, TokenId},
    utils::get_collection_data,
};

const NFT_ADDR: &str = "nft";
const OWNER: &str = "owner";

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
        } => match from_binary::<cw721_base::msg::QueryMsg<Empty>>(&msg).unwrap() {
            QueryMsg::Ownership {} => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&Ownership::<Addr> {
                    owner: Some(Addr::unchecked(OWNER)),
                    pending_owner: None,
                    pending_expiry: None,
                })
                .unwrap(),
            )),
            QueryMsg::AllNftInfo { .. } => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&AllNftInfoResponse::<Option<Empty>> {
                    access: cw721::OwnerOfResponse {
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
            QueryMsg::ContractInfo {} => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&cw721::ContractInfoResponse {
                    name: "name".to_string(),
                    symbol: "symbol".to_string(),
                })
                .unwrap(),
            )),
            QueryMsg::NumTokens {} => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&NumTokensResponse { count: 1 }).unwrap(),
            )),
            _ => unimplemented!(),
        },
        cosmwasm_std::WasmQuery::ContractInfo { .. } => QuerierResult::Ok(ContractResult::Ok(
            to_binary(&ContractInfoResponse {
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
        } => match from_binary::<cw721_base::msg::QueryMsg<Empty>>(&msg).unwrap() {
            // unwrap using latest (not old) cw721-base, since it is backwards compatible
            cw721_base::msg::QueryMsg::Minter {} => QuerierResult::Ok(ContractResult::Ok(
                to_binary(
                    // return v016 response
                    &cw721_base_016::msg::MinterResponse {
                        minter: OWNER.to_string(),
                    },
                )
                .unwrap(),
            )),
            cw721_base::msg::QueryMsg::AllNftInfo { .. } => QuerierResult::Ok(ContractResult::Ok(
                to_binary(
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
            )),
            QueryMsg::ContractInfo {} => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&cw721_016::ContractInfoResponse {
                    name: "name".to_string(),
                    symbol: "symbol".to_string(),
                })
                .unwrap(),
            )),
            QueryMsg::NumTokens {} => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&cw721_016::NumTokensResponse { count: 1 }).unwrap(),
            )),
            _ => QuerierResult::Err(cosmwasm_std::SystemError::Unknown {}), // throws error for Ownership query
        },
        cosmwasm_std::WasmQuery::ContractInfo { .. } => QuerierResult::Ok(ContractResult::Ok(
            to_binary(&ContractInfoResponse {
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
    let expected_contract_info: cosmwasm_std::ContractInfoResponse = from_binary(
        &to_binary(&ContractInfoResponse {
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

        let info = mock_info(NFT_ADDR, &[]);
        let token_id = "1";
        let sender = "ekez".to_string();
        let msg = to_binary(&IbcOutgoingMsg {
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
                info,
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
                data: to_binary(&NonFungibleTokenPacketData {
                    class_id: ClassId::new(NFT_ADDR),
                    class_uri: None,
                    class_data: Some(
                        to_binary(&CollectionData {
                            owner: Some(OWNER.to_string()),
                            contract_info: expected_contract_info.clone(),
                            name: "name".to_string(),
                            symbol: "symbol".to_string(),
                            num_tokens: 1,
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
            .into_iter()
            .collect::<StdResult<Vec<(String, String)>>>()
            .unwrap();
        assert_eq!(keys, [(NFT_ADDR.to_string(), token_id.to_string())]);

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

        let info = mock_info(NFT_ADDR, &[]);
        let token_id = "1";
        let sender = "ekez".to_string();
        let msg = to_binary(&IbcOutgoingMsg {
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
                info,
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
                data: to_binary(&NonFungibleTokenPacketData {
                    class_id: ClassId::new(NFT_ADDR),
                    class_uri: None,
                    class_data: Some(
                        to_binary(&CollectionData {
                            owner: Some(OWNER.to_string()),
                            contract_info: expected_contract_info,
                            name: "name".to_string(),
                            symbol: "symbol".to_string(),
                            num_tokens: 1,
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
            .into_iter()
            .collect::<StdResult<Vec<(String, String)>>>()
            .unwrap();
        assert_eq!(keys, [(NFT_ADDR.to_string(), token_id.to_string())]);

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

        let info = mock_info(NFT_ADDR, &[]);
        let token_id = "1";
        let sender = "ekez".to_string();
        let msg = to_binary(&IbcOutgoingMsg {
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
                info,
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
                data: to_binary(&NonFungibleTokenPacketData {
                    class_id: ClassId::new(NFT_ADDR),
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
            .into_iter()
            .collect::<StdResult<Vec<(String, String)>>>()
            .unwrap();
        assert_eq!(keys, [(NFT_ADDR.to_string(), token_id.to_string())]);

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

    Ics721Contract {}
        .receive_nft(deps.as_mut(), env, info, token_id, sender, msg)
        .unwrap();

    let class = CLASS_ID_TO_CLASS
        .load(deps.as_ref().storage, ClassId::new(NFT_ADDR))
        .unwrap();
    assert_eq!(class.uri, None);
    let expected_contract_info: cosmwasm_std::ContractInfoResponse = from_binary(
        &to_binary(&ContractInfoResponse {
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
            to_binary(&CollectionData {
                owner: Some(OWNER.to_string()),
                contract_info: expected_contract_info,
                name: "name".to_string(),
                symbol: "symbol".to_string(),
                num_tokens: 1,
            })
            .unwrap()
        ),
    );
}
