use cosmwasm_std::{
    testing::{mock_dependencies, mock_info, MockQuerier},
    to_binary, ContractResult, CosmosMsg, Empty, IbcMsg, IbcTimeout, QuerierResult, SubMsg,
    Timestamp, WasmQuery,
};
use cw721::NftInfoResponse;

use crate::{
    contract::receive_nft,
    ibc::NonFungibleTokenPacketData,
    msg::IbcOutgoingMsg,
    state::CLASS_ID_TO_CLASS,
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
