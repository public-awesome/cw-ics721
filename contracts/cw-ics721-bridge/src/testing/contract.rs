use cosmwasm_std::{
    testing::{mock_dependencies, mock_env, mock_info, MockQuerier, MOCK_CONTRACT_ADDR},
    to_binary, ContractResult, CosmosMsg, Empty, IbcMsg, IbcTimeout, Order, QuerierResult,
    StdResult, SubMsg, Timestamp, WasmQuery,
};
use cw721::{AllNftInfoResponse, NftInfoResponse};

use crate::{
    execute::receive_nft,
    ibc::NonFungibleTokenPacketData,
    msg::IbcOutgoingMsg,
    state::Ics721Contract,
    token_types::{ClassId, TokenId},
};

const NFT_ADDR: &str = "nft";

fn nft_info_response_mock_querier(query: &WasmQuery) -> QuerierResult {
    match query {
        cosmwasm_std::WasmQuery::Smart {
            contract_addr,
            msg: _,
        } => {
            if *contract_addr == NFT_ADDR {
                QuerierResult::Ok(ContractResult::Ok(
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

    let res = receive_nft(
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
        SubMsg::new(CosmosMsg::Ibc(IbcMsg::SendPacket {
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
    let keys = Ics721Contract::default()
        .outgoing_class_token_to_channel
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
        Ics721Contract::default()
            .outgoing_class_token_to_channel
            .load(deps.as_mut().storage, key)
            .unwrap(),
        channel_id
    )
}

#[test]
fn test_receive_sets_uri() {
    let mut querier = MockQuerier::default();
    querier.update_wasm(nft_info_response_mock_querier);

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

    receive_nft(deps.as_mut(), env, info, token_id, sender, msg).unwrap();

    let class = Ics721Contract::default()
        .class_id_to_class
        .load(deps.as_ref().storage, ClassId::new(NFT_ADDR))
        .unwrap();
    assert_eq!(class.uri, None);
    assert_eq!(class.data, None);
}
