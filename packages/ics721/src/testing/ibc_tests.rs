use cosmwasm_std::{
    attr,
    testing::{mock_dependencies, mock_env, mock_info, MockQuerier},
    to_binary, to_vec, Addr, Attribute, Binary, ContractResult, DepsMut, Empty, Env,
    IbcAcknowledgement, IbcChannel, IbcChannelConnectMsg, IbcChannelOpenMsg, IbcEndpoint, IbcOrder,
    IbcPacket, IbcPacketReceiveMsg, IbcTimeout, Order, QuerierResult, Reply, Response, StdResult,
    SubMsgResponse, SubMsgResult, Timestamp, WasmQuery,
};

use crate::{
    execute::Ics721Execute,
    ibc::{
        Ics721Ibc, NonFungibleTokenPacketData, ACK_AND_DO_NOTHING, IBC_VERSION,
        INSTANTIATE_CW721_REPLY_ID,
    },
    ibc_helpers::{ack_fail, ack_success, try_get_ack_error},
    msg::{InstantiateMsg, QueryMsg},
    query::Ics721Query,
    state::{
        CollectionData, CLASS_ID_TO_NFT_CONTRACT, INCOMING_CLASS_TOKEN_TO_CHANNEL,
        NFT_CONTRACT_TO_CLASS_ID, PO,
    },
    token_types::{ClassId, TokenId},
    utils::get_collection_data,
    ContractError,
};

const CONTRACT_PORT: &str = "wasm.address1";
const REMOTE_PORT: &str = "stars.address1";
const CONNECTION_ID: &str = "connection-2";
const CHANNEL_ID: &str = "channel-1";
const DEFAULT_TIMEOUT: u64 = 42; // Seconds.

const ADDR1: &str = "addr1";
const RELAYER_ADDR: &str = "relayer";
const CW721_CODE_ID: u64 = 0;

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

fn mock_channel(channel_id: &str) -> IbcChannel {
    IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{channel_id}5"),
        },
        IbcOrder::Unordered,
        IBC_VERSION,
        CONNECTION_ID,
    )
}

fn mock_packet(data: Binary) -> IbcPacket {
    IbcPacket::new(
        data,
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: CHANNEL_ID.to_string(),
        },
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: CHANNEL_ID.to_string(),
        },
        42, // Packet sequence number.
        IbcTimeout::with_timestamp(Timestamp::from_seconds(DEFAULT_TIMEOUT)),
    )
}

fn add_channel(mut deps: DepsMut, env: Env, channel_id: &str) {
    let channel = mock_channel(channel_id);
    let open_msg = IbcChannelOpenMsg::new_init(channel.clone());
    Ics721Contract::default()
        .ibc_channel_open(deps.branch(), env.clone(), open_msg)
        .unwrap();
    let connect_msg = IbcChannelConnectMsg::new_ack(channel.clone(), IBC_VERSION);
    let res = Ics721Contract::default()
        .ibc_channel_connect(deps.branch(), env.clone(), connect_msg)
        .unwrap();

    // Smoke check our attributes
    assert_eq!(res.attributes.len(), 3);
    assert_eq!(
        res.attributes,
        vec![
            attr("method", "ibc_channel_connect"),
            attr("channel", channel.endpoint.channel_id),
            attr("port", channel.endpoint.port_id)
        ]
    );
    assert_eq!(res.messages.len(), 0);
}

fn do_instantiate(deps: DepsMut, env: Env, sender: &str) -> StdResult<Response> {
    let msg = InstantiateMsg {
        cw721_base_code_id: CW721_CODE_ID,
        proxy: None,
        pauser: None,
    };
    Ics721Contract::default().instantiate(deps, env, mock_info(sender, &[]), msg)
}

#[allow(clippy::too_many_arguments)]
fn build_ics_packet(
    class_id: &str,
    class_uri: Option<&str>,
    class_data: Option<Binary>,
    token_ids: Vec<&str>,
    token_uris: Option<Vec<&str>>,
    token_data: Option<Vec<Binary>>,
    sender: &str,
    receiver: &str,
    memo: Option<&str>,
) -> NonFungibleTokenPacketData {
    NonFungibleTokenPacketData {
        class_id: ClassId::new(class_id),
        class_uri: class_uri.map(|s| s.to_string()),
        class_data,
        token_ids: token_ids.into_iter().map(TokenId::new).collect(),
        token_uris: token_uris.map(|t| t.into_iter().map(|s| s.to_string()).collect()),
        token_data,
        sender: sender.to_string(),
        receiver: receiver.to_string(),
        memo: memo.map(|t| t.to_string()),
    }
}

#[test]
fn test_reply_cw721() {
    let mut querier = MockQuerier::default();
    querier.update_wasm(|query| -> QuerierResult {
        match query {
            WasmQuery::Smart {
                contract_addr: _,
                msg: _,
            } => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&cw721::ContractInfoResponse {
                    name: "wasm.address1/channel-10/address2".to_string(),
                    symbol: "wasm.address1/channel-10/address2".to_string(),
                })
                .unwrap(),
            )),
            WasmQuery::Raw { .. } => QuerierResult::Ok(ContractResult::Ok(Binary::default())),
            WasmQuery::ContractInfo { .. } => {
                QuerierResult::Ok(ContractResult::Ok(Binary::default()))
            }
            _ => QuerierResult::Ok(ContractResult::Ok(Binary::default())),
        }
    });
    let mut deps = mock_dependencies();
    deps.querier = querier;

    // This is a pre encoded message with the contract address
    // cosmos2contract
    // TODO: Can we form this via a function instead of hardcoding
    //       So we can have different contract addresses
    let reply_resp = "Cg9jb3Ntb3MyY29udHJhY3QSAA==";
    let rep = Reply {
        id: INSTANTIATE_CW721_REPLY_ID,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(Binary::from_base64(reply_resp).unwrap()),
        }),
    };

    let res = Ics721Contract::default()
        .reply(deps.as_mut(), mock_env(), rep)
        .unwrap();
    // assert_eq!(res.data, Some(ack_success()));
    assert_eq!(
        res.attributes,
        vec![
            attr("method", "instantiate_cw721_reply"),
            attr("class_id", "wasm.address1/channel-10/address2"),
            attr("cw721_addr", "cosmos2contract")
        ]
    );

    let class_id = NFT_CONTRACT_TO_CLASS_ID
        .load(deps.as_ref().storage, Addr::unchecked("cosmos2contract"))
        .unwrap();
    let nft = CLASS_ID_TO_NFT_CONTRACT
        .load(deps.as_ref().storage, class_id.clone())
        .unwrap();

    assert_eq!(nft, Addr::unchecked("cosmos2contract"));
    assert_eq!(class_id.to_string(), "wasm.address1/channel-10/address2");
}

#[test]
fn test_stateless_reply() {
    let mut deps = mock_dependencies();

    let rep = Reply {
        id: ACK_AND_DO_NOTHING,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: None,
        }),
    };
    let res = Ics721Contract::default()
        .reply(deps.as_mut(), mock_env(), rep)
        .unwrap();
    assert_eq!(res.data, Some(ack_success()));

    let rep = Reply {
        id: ACK_AND_DO_NOTHING,
        result: SubMsgResult::Err("some failure".to_string()),
    };
    let res = Ics721Contract::default()
        .reply(deps.as_mut(), mock_env(), rep)
        .unwrap();
    assert_eq!(res.data, Some(ack_fail("some failure".to_string())));
}

#[test]
fn test_unrecognised_reply() {
    let mut deps = mock_dependencies();
    let rep = Reply {
        id: 420,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: None,
        }),
    };
    let err = Ics721Contract::default()
        .reply(deps.as_mut(), mock_env(), rep)
        .unwrap_err();
    assert_eq!(err, ContractError::UnrecognisedReplyId {})
}

#[test]
fn test_ibc_channel_open() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    // Add channel calls open and connect valid
    add_channel(deps.as_mut(), env, "channel-1");
}

#[test]
#[should_panic(expected = "OrderedChannel")]
fn test_ibc_channel_open_ordered_channel() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    let channel_id = "channel-1";
    let channel = IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{channel_id}5"),
        },
        IbcOrder::Ordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelOpenMsg::OpenInit { channel };
    Ics721Contract::default()
        .ibc_channel_open(deps.as_mut(), env, msg)
        .unwrap();
}

#[test]
#[should_panic(expected = "InvalidVersion { actual: \"invalid_version\", expected: \"ics721-1\" }")]
fn test_ibc_channel_open_invalid_version() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    let channel_id = "channel-1";
    let channel = IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{channel_id}5"),
        },
        IbcOrder::Unordered,
        "invalid_version",
        CONNECTION_ID,
    );

    let msg = IbcChannelOpenMsg::OpenInit { channel };
    Ics721Contract::default()
        .ibc_channel_open(deps.as_mut(), env, msg)
        .unwrap();
}

#[test]
#[should_panic(expected = "InvalidVersion { actual: \"invalid_version\", expected: \"ics721-1\" }")]
fn test_ibc_channel_open_invalid_version_counterparty() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    let channel_id = "channel-1";
    let channel = IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{channel_id}5"),
        },
        IbcOrder::Unordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelOpenMsg::OpenTry {
        channel,
        counterparty_version: "invalid_version".to_string(),
    };
    Ics721Contract::default()
        .ibc_channel_open(deps.as_mut(), env, msg)
        .unwrap();
}

#[test]
fn test_ibc_channel_connect() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    // Add channel calls open and connect valid
    add_channel(deps.as_mut(), env, "channel-1");
}

#[test]
#[should_panic(expected = "OrderedChannel")]
fn test_ibc_channel_connect_ordered_channel() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    let channel_id = "channel-1";
    let channel = IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{channel_id}5"),
        },
        IbcOrder::Ordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelConnectMsg::new_confirm(channel);
    Ics721Contract::default()
        .ibc_channel_connect(deps.as_mut(), env, msg)
        .unwrap();
}

#[test]
#[should_panic(expected = "InvalidVersion { actual: \"invalid_version\", expected: \"ics721-1\" }")]
fn test_ibc_channel_connect_invalid_version() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    let channel_id = "channel-1";
    let channel = IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{channel_id}5"),
        },
        IbcOrder::Unordered,
        "invalid_version",
        CONNECTION_ID,
    );

    let msg = IbcChannelConnectMsg::OpenConfirm { channel };
    Ics721Contract::default()
        .ibc_channel_connect(deps.as_mut(), env, msg)
        .unwrap();
}

#[test]
#[should_panic(expected = "InvalidVersion { actual: \"invalid_version\", expected: \"ics721-1\" }")]
fn test_ibc_channel_connect_invalid_version_counterparty() {
    let mut deps = mock_dependencies();
    let env = mock_env();

    // Instantiate the contract
    do_instantiate(deps.as_mut(), env.clone(), ADDR1).unwrap();

    let channel_id = "channel-1";
    let channel = IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{channel_id}5"),
        },
        IbcOrder::Unordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelConnectMsg::OpenAck {
        channel,
        counterparty_version: "invalid_version".to_string(),
    };
    Ics721Contract::default()
        .ibc_channel_connect(deps.as_mut(), env, msg)
        .unwrap();
}

#[test]
fn test_ibc_packet_receive() {
    let data = to_binary(&NonFungibleTokenPacketData {
        class_id: ClassId::new("id"),
        class_uri: None,
        class_data: None,
        token_ids: vec![TokenId::new("1")],
        token_uris: None,
        token_data: None,
        sender: "violet".to_string(),
        receiver: "blue".to_string(),
        memo: None,
    })
    .unwrap();
    let ibc_packet = mock_packet(data);
    let packet = IbcPacketReceiveMsg::new(ibc_packet.clone(), Addr::unchecked(RELAYER_ADDR));
    let mut deps = mock_dependencies();
    let env = mock_env();
    PO.set_pauser(&mut deps.storage, &deps.api, None).unwrap();
    Ics721Contract::default()
        .ibc_packet_receive(deps.as_mut(), env, packet)
        .unwrap();

    // check incoming classID and tokenID
    let keys = INCOMING_CLASS_TOKEN_TO_CHANNEL
        .keys(deps.as_mut().storage, None, None, Order::Ascending)
        .into_iter()
        .collect::<StdResult<Vec<(String, String)>>>()
        .unwrap();
    let class_id = format!(
        "{}/{}/{}",
        ibc_packet.dest.port_id, ibc_packet.dest.channel_id, "id"
    );
    assert_eq!(keys, [(class_id, "1".to_string())]);

    // check channel
    let key = (
        ClassId::new(keys[0].clone().0),
        TokenId::new(keys[0].clone().1),
    );
    assert_eq!(
        INCOMING_CLASS_TOKEN_TO_CHANNEL
            .load(deps.as_mut().storage, key)
            .unwrap(),
        ibc_packet.dest.channel_id,
    )
}

#[test]
fn test_ibc_packet_receive_invalid_packet_data() {
    // the actual message used here is unimportant. this just
    // constructs a valud JSON blob that is not a valid ICS-721
    // packet.
    let data = to_binary(&QueryMsg::ClassMetadata {
        class_id: "foobar".to_string(),
    })
    .unwrap();

    let packet = IbcPacketReceiveMsg::new(mock_packet(data), Addr::unchecked(RELAYER_ADDR));
    let mut deps = mock_dependencies();
    let env = mock_env();

    PO.set_pauser(&mut deps.storage, &deps.api, None).unwrap();

    let res = Ics721Contract::default().ibc_packet_receive(deps.as_mut(), env, packet);

    assert!(res.is_ok());
    let error = try_get_ack_error(&IbcAcknowledgement::new(res.unwrap().acknowledgement));

    assert!(error
        .unwrap()
        .starts_with("Error parsing into type ics721::ibc::NonFungibleTokenPacketData"))
}

#[test]
fn test_ibc_packet_receive_emits_memo() {
    let data = to_binary(&NonFungibleTokenPacketData {
        class_id: ClassId::new("id"),
        class_uri: None,
        class_data: None,
        token_ids: vec![TokenId::new("1")],
        token_uris: None,
        token_data: None,
        sender: "violet".to_string(),
        receiver: "blue".to_string(),
        memo: Some("memo".to_string()),
    })
    .unwrap();
    let packet = IbcPacketReceiveMsg::new(mock_packet(data), Addr::unchecked(RELAYER_ADDR));
    let mut deps = mock_dependencies();
    let env = mock_env();
    PO.set_pauser(&mut deps.storage, &deps.api, None).unwrap();
    let res = Ics721Contract::default()
        .ibc_packet_receive(deps.as_mut(), env, packet)
        .unwrap();
    assert!(res.attributes.contains(&Attribute {
        key: "ics721_memo".to_string(),
        value: "memo".to_string()
    }))
}

#[test]
fn test_ibc_packet_receive_missmatched_lengths() {
    let mut deps = mock_dependencies();

    PO.set_pauser(&mut deps.storage, &deps.api, None).unwrap();

    // More URIs are provided than tokens.
    let data = build_ics_packet(
        "bad kids",
        None,
        None,
        vec!["kid A"],
        Some(vec!["a", "b"]),
        None,
        "ekez",
        "callum",
        None,
    );

    let packet = IbcPacketReceiveMsg::new(
        mock_packet(to_binary(&data).unwrap()),
        Addr::unchecked(RELAYER_ADDR),
    );

    let res = Ics721Contract::default().ibc_packet_receive(deps.as_mut(), mock_env(), packet);

    assert!(res.is_ok());
    let error = try_get_ack_error(&IbcAcknowledgement::new(res.unwrap().acknowledgement));

    assert_eq!(
        error,
        Some(ContractError::TokenInfoLenMissmatch {}.to_string())
    );

    // More token data are provided than tokens.
    let token_data = Some(vec![
        to_binary("some_data_1").unwrap(),
        to_binary("some_data_2").unwrap(),
    ]);
    let data = build_ics_packet(
        "bad kids",
        None,
        None,
        vec!["kid A"],
        Some(vec!["a"]),
        token_data,
        "ekez",
        "callum",
        None,
    );

    let packet = IbcPacketReceiveMsg::new(
        mock_packet(to_binary(&data).unwrap()),
        Addr::unchecked(RELAYER_ADDR),
    );

    let res = Ics721Contract::default().ibc_packet_receive(deps.as_mut(), mock_env(), packet);

    assert!(res.is_ok());
    let error = try_get_ack_error(&IbcAcknowledgement::new(res.unwrap().acknowledgement));

    assert_eq!(
        error,
        Some(ContractError::TokenInfoLenMissmatch {}.to_string())
    )
}

#[test]
fn test_packet_json() {
    let class_data = to_binary("some_class_data").unwrap(); // InNvbWVfY2xhc3NfZGF0YSI=
    let token_data = vec![
        // ["InNvbWVfdG9rZW5fZGF0YV8xIg==","InNvbWVfdG9rZW5fZGF0YV8yIg==","
        // InNvbWVfdG9rZW5fZGF0YV8zIg=="]
        to_binary("some_token_data_1").unwrap(),
        to_binary("some_token_data_2").unwrap(),
        to_binary("some_token_data_3").unwrap(),
    ];
    let packet = build_ics_packet(
        "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n",
        Some("https://metadata-url.com/my-metadata"),
        Some(class_data),
        vec!["1", "2", "3"],
        Some(vec![
            "https://metadata-url.com/my-metadata1",
            "https://metadata-url.com/my-metadata2",
            "https://metadata-url.com/my-metadata3",
        ]),
        Some(token_data),
        "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n",
        "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc",
        Some("some_memo"),
    );
    // Example message generated from the SDK
    // TODO: test with non-null tokenData and classData.
    let expected = r#"{"classId":"stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n","classUri":"https://metadata-url.com/my-metadata","classData":"InNvbWVfY2xhc3NfZGF0YSI=","tokenIds":["1","2","3"],"tokenUris":["https://metadata-url.com/my-metadata1","https://metadata-url.com/my-metadata2","https://metadata-url.com/my-metadata3"],"tokenData":["InNvbWVfdG9rZW5fZGF0YV8xIg==","InNvbWVfdG9rZW5fZGF0YV8yIg==","InNvbWVfdG9rZW5fZGF0YV8zIg=="],"sender":"stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n","receiver":"wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc","memo":"some_memo"}"#;

    let encdoded = String::from_utf8(to_vec(&packet).unwrap()).unwrap();
    assert_eq!(expected, encdoded.as_str());
}

#[test]
fn test_no_receive_when_paused() {
    // Valid JSON, invalid ICS-721 packet. Tests that we check for
    // pause status before attempting validation.
    let data = to_binary(&QueryMsg::ClassMetadata {
        class_id: "foobar".to_string(),
    })
    .unwrap();

    let packet = IbcPacketReceiveMsg::new(mock_packet(data), Addr::unchecked(RELAYER_ADDR));
    let mut deps = mock_dependencies();
    let env = mock_env();

    PO.set_pauser(&mut deps.storage, &deps.api, Some("ekez"))
        .unwrap();
    PO.pause(&mut deps.storage, &Addr::unchecked("ekez"))
        .unwrap();

    let res = Ics721Contract::default().ibc_packet_receive(deps.as_mut(), env, packet);

    assert!(res.is_ok());
    let error = try_get_ack_error(&IbcAcknowledgement::new(res.unwrap().acknowledgement));

    assert!(error
        .unwrap()
        .starts_with("contract is paused pending governance intervention"))
}
