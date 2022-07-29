use cosmwasm_std::{
    attr,
    testing::{mock_dependencies, mock_env, mock_info, MockQuerier},
    to_binary, to_vec, Binary, ContractResult, DepsMut, Env, IbcAcknowledgement, IbcChannel,
    IbcChannelConnectMsg, IbcChannelOpenMsg, IbcEndpoint, IbcOrder, IbcPacket, IbcPacketReceiveMsg,
    IbcTimeout, QuerierResult, Reply, Response, SubMsg, SubMsgResponse, SubMsgResult, Timestamp,
    WasmMsg, WasmQuery,
};

use crate::{
    contract::instantiate,
    helpers::{
        BATCH_TRANSFER_FROM_CHANNEL_REPLY_ID, BURN_ESCROW_TOKENS_REPLY_ID,
        FAILURE_RESPONSE_FAILURE_REPLY_ID, INSTANTIATE_AND_MINT_CW721_REPLY_ID,
        INSTANTIATE_CW721_REPLY_ID, INSTANTIATE_ESCROW_REPLY_ID, MINT_SUB_MSG_REPLY_ID,
    },
    ibc::{
        ibc_channel_connect, ibc_channel_open, ibc_packet_receive, reply,
        NonFungibleTokenPacketData, IBC_VERSION,
    },
    ibc_helpers::{ack_fail, ack_success, try_get_ack_error},
    msg::{ExecuteMsg, InstantiateMsg, QueryMsg},
    ContractError,
};

const CONTRACT_PORT: &str = "wasm.address1";
const REMOTE_PORT: &str = "stars.address1";
const CONNECTION_ID: &str = "connection-2";
const CHANNEL_ID: &str = "channel-1";
const DEFAULT_TIMEOUT: u64 = 42; // Seconds.

const ADDR1: &str = "addr1";
const CW721_CODE_ID: u64 = 0;
const ESCROW_CODE_ID: u64 = 1;

fn mock_channel(channel_id: &str) -> IbcChannel {
    IbcChannel::new(
        IbcEndpoint {
            port_id: CONTRACT_PORT.to_string(),
            channel_id: channel_id.to_string(),
        },
        IbcEndpoint {
            port_id: REMOTE_PORT.to_string(),
            channel_id: format!("{}5", channel_id),
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
    ibc_channel_open(deps.branch(), env.clone(), open_msg).unwrap();
    let connect_msg = IbcChannelConnectMsg::new_ack(channel.clone(), IBC_VERSION);
    let res = ibc_channel_connect(deps.branch(), env, connect_msg).unwrap();

    // Sanity check our attributes
    assert_eq!(res.attributes.len(), 3);
    assert_eq!(
        res.attributes,
        vec![
            attr("method", "ibc_channel_connect"),
            attr("channel", channel.endpoint.channel_id),
            attr("port", channel.endpoint.port_id)
        ]
    );
    assert_eq!(res.messages.len(), 1);
    assert_eq!(
        res.messages[0],
        SubMsg::reply_always(
            WasmMsg::Instantiate {
                admin: None,
                code_id: ESCROW_CODE_ID,
                msg: to_binary(&ics721_escrow::msg::InstantiateMsg {
                    admin_address: "cosmos2contract".to_string(),
                    channel_id: channel_id.to_string()
                })
                .unwrap(),
                funds: vec![],
                label: format!("channel ({}) ICS721 escrow", channel_id)
            },
            INSTANTIATE_ESCROW_REPLY_ID
        )
    )
}

fn do_instantiate(deps: DepsMut, env: Env, sender: &str) -> Result<Response, ContractError> {
    let msg = InstantiateMsg {
        cw721_ics_code_id: CW721_CODE_ID,
        escrow_code_id: ESCROW_CODE_ID,
    };
    instantiate(deps, env, mock_info(sender, &[]), msg)
}

fn build_ics_packet(
    class_id: &str,
    class_uri: Option<&str>,
    token_ids: Vec<&str>,
    token_uris: Vec<&str>,
    sender: &str,
    receiver: &str,
) -> NonFungibleTokenPacketData {
    NonFungibleTokenPacketData {
        class_id: class_id.to_string(),
        class_uri: class_uri.map(|s| s.to_string()),
        token_ids: token_ids.into_iter().map(|s| s.to_string()).collect(),
        token_uris: token_uris.into_iter().map(|s| s.to_string()).collect(),
        sender: sender.to_string(),
        receiver: receiver.to_string(),
    }
}

#[test]
fn test_reply_escrow() {
    let mut querier = MockQuerier::default();
    querier.update_wasm(|query| -> QuerierResult {
        match query {
            WasmQuery::Smart {
                contract_addr: _,
                msg: _,
            } => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&"channel-1".to_string()).unwrap(),
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
        id: INSTANTIATE_ESCROW_REPLY_ID,
        result: SubMsgResult::Ok(SubMsgResponse {
            events: vec![],
            data: Some(Binary::from_base64(reply_resp).unwrap()),
        }),
    };
    let res = reply(deps.as_mut(), mock_env(), rep).unwrap();
    assert_eq!(res.data, Some(ack_success()));
    assert_eq!(
        res.attributes,
        vec![
            attr("method", "instantiate_escrow_reply"),
            attr("escrow_addr", "cosmos2contract"),
            attr("channel_id", "channel-1")
        ]
    );
}

#[test]
fn test_reply_escrow_submsg_fail() {
    let mut querier = MockQuerier::default();
    querier.update_wasm(|query| -> QuerierResult {
        match query {
            WasmQuery::Smart {
                contract_addr: _,
                msg: _,
            } => QuerierResult::Ok(ContractResult::Ok(
                to_binary(&"channel-1".to_string()).unwrap(),
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

    // The instantiate has failed for some reason
    let rep = Reply {
        id: INSTANTIATE_ESCROW_REPLY_ID,
        result: SubMsgResult::Err("some failure".to_string()),
    };
    let res = reply(deps.as_mut(), mock_env(), rep).unwrap();
    assert_eq!(res.data, Some(ack_fail("some failure").unwrap()));
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
    let res = reply(deps.as_mut(), mock_env(), rep).unwrap();
    // assert_eq!(res.data, Some(ack_success()));
    assert_eq!(
        res.attributes,
        vec![
            attr("method", "instantiate_cw721_reply"),
            attr("class_id", "wasm.address1/channel-10/address2"),
            attr("cw721_addr", "cosmos2contract")
        ]
    );
}

#[test]
fn test_stateless_reply() {
    let mut deps = mock_dependencies();
    // List of all our stateless replies, we can test them all in one
    let reply_ids = vec![
        MINT_SUB_MSG_REPLY_ID,
        INSTANTIATE_AND_MINT_CW721_REPLY_ID,
        BATCH_TRANSFER_FROM_CHANNEL_REPLY_ID,
        BURN_ESCROW_TOKENS_REPLY_ID,
        FAILURE_RESPONSE_FAILURE_REPLY_ID,
    ];

    // Success case
    for id in &reply_ids {
        let rep = Reply {
            id: *id,
            result: SubMsgResult::Ok(SubMsgResponse {
                events: vec![],
                data: None,
            }),
        };
        let res = reply(deps.as_mut(), mock_env(), rep).unwrap();
        assert_eq!(res.data, Some(ack_success()));
    }

    // Error case
    for id in &reply_ids {
        let rep = Reply {
            id: *id,
            result: SubMsgResult::Err("some failure".to_string()),
        };
        let res = reply(deps.as_mut(), mock_env(), rep).unwrap();
        assert_eq!(res.data, Some(ack_fail("some failure").unwrap()));
    }
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
    reply(deps.as_mut(), mock_env(), rep).unwrap_err();
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
            channel_id: format!("{}5", channel_id),
        },
        IbcOrder::Ordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelOpenMsg::OpenInit { channel };
    ibc_channel_open(deps.as_mut(), env, msg).unwrap();
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
            channel_id: format!("{}5", channel_id),
        },
        IbcOrder::Unordered,
        "invalid_version",
        CONNECTION_ID,
    );

    let msg = IbcChannelOpenMsg::OpenInit { channel };
    ibc_channel_open(deps.as_mut(), env, msg).unwrap();
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
            channel_id: format!("{}5", channel_id),
        },
        IbcOrder::Unordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelOpenMsg::OpenTry {
        channel,
        counterparty_version: "invalid_version".to_string(),
    };
    ibc_channel_open(deps.as_mut(), env, msg).unwrap();
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
            channel_id: format!("{}5", channel_id),
        },
        IbcOrder::Ordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelConnectMsg::new_confirm(channel);
    ibc_channel_connect(deps.as_mut(), env, msg).unwrap();
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
            channel_id: format!("{}5", channel_id),
        },
        IbcOrder::Unordered,
        "invalid_version",
        CONNECTION_ID,
    );

    let msg = IbcChannelConnectMsg::OpenConfirm { channel };
    ibc_channel_connect(deps.as_mut(), env, msg).unwrap();
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
            channel_id: format!("{}5", channel_id),
        },
        IbcOrder::Unordered,
        IBC_VERSION,
        CONNECTION_ID,
    );

    let msg = IbcChannelConnectMsg::OpenAck {
        channel,
        counterparty_version: "invalid_version".to_string(),
    };
    ibc_channel_connect(deps.as_mut(), env, msg).unwrap();
}

#[test]
fn test_ibc_packet_receive_invalid_packet_data() {
    let data = to_binary(&QueryMsg::ListChannels {
        start_after: None,
        limit: None,
    })
    .unwrap();

    let packet = IbcPacketReceiveMsg::new(mock_packet(data));
    let mut deps = mock_dependencies();
    let env = mock_env();

    let res = ibc_packet_receive(deps.as_mut(), env, packet);

    assert!(res.is_ok());
    let error = try_get_ack_error(&IbcAcknowledgement::new(res.unwrap().acknowledgement)).unwrap();

    assert_eq!(
        error,
        Some("Error parsing into type cw_ics721_bridge::ibc::NonFungibleTokenPacketData: missing field `classId`".to_string())
    )
}

#[test]
fn test_ibc_packet_receive_missmatched_lengths() {
    let data = build_ics_packet("bad kids", None, vec!["kid A"], vec![], "ekez", "callum");

    let packet = IbcPacketReceiveMsg::new(mock_packet(to_binary(&data).unwrap()));
    let mut deps = mock_dependencies();
    let env = mock_env();

    let res = ibc_packet_receive(deps.as_mut(), env, packet);

    assert!(res.is_ok());
    let error = try_get_ack_error(&IbcAcknowledgement::new(res.unwrap().acknowledgement)).unwrap();

    assert_eq!(
        error,
        Some("tokenId list has different length than tokenUri list".to_string())
    )
}

#[test]
fn ibc_receive_source_chain() {
    // We should pop from this packet when receiving it as it is
    // prefixed with the receivers info.
    let there_and_back_again = format!(
        "{}/{}/nft-transfer/channel-42/{}",
        REMOTE_PORT, CHANNEL_ID, ADDR1
    );

    let data = build_ics_packet(
        &there_and_back_again,
        None,
        vec!["kid A"],
        vec!["https://moonphase.is/image.svg"],
        "ekez",
        "callum",
    );

    let packet = IbcPacketReceiveMsg::new(mock_packet(to_binary(&data).unwrap()));
    let mut deps = mock_dependencies();
    let env = mock_env();

    let res = ibc_packet_receive(deps.as_mut(), env.clone(), packet);

    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.messages.len(), 1);
    let response = res.messages.into_iter().next().unwrap();

    assert_eq!(
        response,
        SubMsg::reply_always(
            WasmMsg::Execute {
                contract_addr: env.contract.address.into_string(),
                msg: to_binary(&ExecuteMsg::BatchTransferFromChannel {
                    channel: CHANNEL_ID.to_string(),
                    // Should have popped from the class ID.
                    class_id: format!("nft-transfer/channel-42/{}", ADDR1),
                    token_ids: vec!["kid A".to_string()],
                    receiver: "callum".to_string()
                })
                .unwrap(),
                funds: vec![]
            },
            BATCH_TRANSFER_FROM_CHANNEL_REPLY_ID
        )
    )
}

#[test]
fn ibc_receive_sink_chain() {
    // We should push from this packet when receiving it as it is
    // not prefixed with the receivers info.
    let back_again = "bleb/belb/nft-transfer/channel-42/{}".to_string();

    let data = build_ics_packet(
        &back_again,
        None,
        vec!["kid A"],
        vec!["https://moonphase.is/image.svg"],
        "ekez",
        "callum",
    );

    let packet = IbcPacketReceiveMsg::new(mock_packet(to_binary(&data).unwrap()));
    let mut deps = mock_dependencies();
    let env = mock_env();

    let res = ibc_packet_receive(deps.as_mut(), env.clone(), packet);

    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.messages.len(), 1);
    let response = res.messages.into_iter().next().unwrap();

    assert_eq!(
        response,
        SubMsg::reply_always(
            WasmMsg::Execute {
                contract_addr: env.contract.address.into_string(),
                msg: to_binary(&ExecuteMsg::DoInstantiateAndMint {
                    class_id: format!("{}/{}/{}", CONTRACT_PORT, CHANNEL_ID, back_again),
                    class_uri: None,
                    token_ids: vec!["kid A".to_string()],
                    token_uris: vec!["https://moonphase.is/image.svg".to_string()],
                    receiver: "callum".to_string()
                })
                .unwrap(),
                funds: vec![]
            },
            INSTANTIATE_AND_MINT_CW721_REPLY_ID
        )
    )
}

#[test]
fn ibc_receive_sink_chain_empty_class_id() {
    // We should push from this packet when receiving it as it is
    // not prefixed with the receivers info.
    let back_again = "".to_string();

    let data = build_ics_packet(
        &back_again,
        None,
        vec!["kid A"],
        vec!["https://moonphase.is/image.svg"],
        "ekez",
        "callum",
    );

    let packet = IbcPacketReceiveMsg::new(mock_packet(to_binary(&data).unwrap()));
    let mut deps = mock_dependencies();
    let env = mock_env();

    let res = ibc_packet_receive(deps.as_mut(), env.clone(), packet);

    assert!(res.is_ok());

    let res = res.unwrap();
    assert_eq!(res.messages.len(), 1);
    let response = res.messages.into_iter().next().unwrap();

    assert_eq!(
        response,
        SubMsg::reply_always(
            WasmMsg::Execute {
                contract_addr: env.contract.address.into_string(),
                msg: to_binary(&ExecuteMsg::DoInstantiateAndMint {
                    class_id: format!("{}/{}/{}", CONTRACT_PORT, CHANNEL_ID, back_again),
                    class_uri: None,
                    token_ids: vec!["kid A".to_string()],
                    token_uris: vec!["https://moonphase.is/image.svg".to_string()],
                    receiver: "callum".to_string()
                })
                .unwrap(),
                funds: vec![]
            },
            INSTANTIATE_AND_MINT_CW721_REPLY_ID
        )
    )
}

#[test]
fn test_packet_json() {
    let packet = build_ics_packet(
        "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n",
        Some("https://metadata-url.com/my-metadata"),
        vec!["1", "2", "3"],
        vec![
            "https://metadata-url.com/my-metadata1",
            "https://metadata-url.com/my-metadata2",
            "https://metadata-url.com/my-metadata3",
        ],
        "stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n",
        "wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc",
    );
    // Example message generated from the SDK
    let expected = r#"{"classId":"stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n","classUri":"https://metadata-url.com/my-metadata","tokenIds":["1","2","3"],"tokenUris":["https://metadata-url.com/my-metadata1","https://metadata-url.com/my-metadata2","https://metadata-url.com/my-metadata3"],"sender":"stars1zedxv25ah8fksmg2lzrndrpkvsjqgk4zt5ff7n","receiver":"wasm1fucynrfkrt684pm8jrt8la5h2csvs5cnldcgqc"}"#;

    let encdoded = String::from_utf8(to_vec(&packet).unwrap()).unwrap();
    assert_eq!(expected, encdoded.as_str());
}
