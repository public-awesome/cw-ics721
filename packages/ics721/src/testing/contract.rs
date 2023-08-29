use cosmwasm_std::{
    from_binary,
    testing::{mock_dependencies, mock_env, mock_info, MockQuerier, MOCK_CONTRACT_ADDR},
    to_binary, Addr, ContractResult, CosmosMsg, Empty, IbcMsg, IbcTimeout, Order, QuerierResult,
    StdResult, SubMsg, Timestamp, WasmQuery,
};
use cw721::{AllNftInfoResponse, NftInfoResponse};
use cw721_base::QueryMsg;
use cw_ownable::Ownership;

use crate::{
    execute::Ics721Execute,
    ibc::{Ics721Ibc, NonFungibleTokenPacketData},
    msg::IbcOutgoingMsg,
    query::Ics721Query,
    state::{ClassData, CLASS_ID_TO_CLASS, OUTGOING_CLASS_TOKEN_TO_CHANNEL},
    token_types::{ClassId, TokenId},
};

const NFT_ADDR: &str = "nft";
const OWNER: &str = "owner";

#[derive(Default)]
pub struct Ics721Contract {}
impl Ics721Execute<Empty> for Ics721Contract {}
impl Ics721Ibc<Empty> for Ics721Contract {}
impl Ics721Query for Ics721Contract {}

fn mock_querier(query: &WasmQuery) -> QuerierResult {
    match query {
        cosmwasm_std::WasmQuery::Smart {
            contract_addr: _,
            msg,
        } => {
            let cw721_base_query_msg = from_binary::<cw721_base::msg::QueryMsg<Empty>>(&msg);
            let cw721_legacy_minter_query_msg =
                from_binary::<cw721_base_016::msg::QueryMsg<Empty>>(&msg);
            match (cw721_base_query_msg, cw721_legacy_minter_query_msg) {
                (Ok(msg), _) => match msg {
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
                    _ => unimplemented!(),
                },
                (_, Ok(_)) => unimplemented!(),
                (_, _) => unimplemented!(),
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
                    to_binary(&ClassData {
                        owner: Some(OWNER.to_string())
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
    assert_eq!(
        class.data,
        Some(
            to_binary(&ClassData {
                owner: Some(OWNER.to_string())
            })
            .unwrap()
        ),
    );
}
