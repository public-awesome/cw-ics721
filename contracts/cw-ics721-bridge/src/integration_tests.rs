use cosmwasm_std::{to_binary, Addr, Empty, IbcTimeout, IbcTimeoutBlock};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

use crate::{
    msg::{CallbackMsg, ExecuteMsg, IbcAwayMsg, InstantiateMsg, QueryMsg},
    ContractError,
};

const COMMUNITY_POOL: &str = "community_pool";

fn cw721_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_base::entry::execute,
        cw721_base::entry::instantiate,
        cw721_base::entry::query,
    );
    Box::new(contract)
}

fn bridge_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    )
    .with_reply(crate::ibc::reply);
    Box::new(contract)
}

fn instantiate_bridge(app: &mut App) -> Addr {
    let cw721_id = app.store_code(cw721_contract());
    let bridge_id = app.store_code(bridge_contract());

    app.instantiate_contract(
        bridge_id,
        Addr::unchecked(COMMUNITY_POOL),
        &InstantiateMsg {
            cw721_base_code_id: cw721_id,
            proxy: None,
            pauser: None,
        },
        &[],
        "cw-ics721-bridge",
        None,
    )
    .unwrap()
}

fn instantiate_bridge_with_proxy(app: &mut App, proxy: Option<String>) -> Addr {
    let cw721_id = app.store_code(cw721_contract());
    let bridge_id = app.store_code(bridge_contract());

    app.instantiate_contract(
        bridge_id,
        Addr::unchecked(COMMUNITY_POOL),
        &InstantiateMsg {
            cw721_base_code_id: cw721_id,
            proxy,
            pauser: None,
        },
        &[],
        "cw-ics721-bridge",
        None,
    )
    .unwrap()
}

#[test]
fn test_instantiate() {
    let mut app = App::default();

    instantiate_bridge(&mut app);
}

#[test]
fn test_do_instantiate_and_mint_weird_data() {
    let mut app = App::default();

    let bridge = instantiate_bridge(&mut app);

    app.execute_contract(
        bridge.clone(),
        bridge,
        &ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
            class_id: "bad kids".to_string(),
            class_uri: None,
            token_ids: vec!["1".to_string()],
            token_uris: vec!["".to_string()], // Empty string should be allowed.
            receiver: "ekez".to_string(),
        }),
        &[],
    )
    .unwrap();
}

#[test]
fn test_do_instantiate_and_mint() {
    let mut app = App::default();

    let bridge = instantiate_bridge(&mut app);

    app.execute_contract(
        bridge.clone(),
        bridge.clone(),
        &ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
            class_id: "bad kids".to_string(),
            class_uri: Some("https://moonphase.is".to_string()),
            token_ids: vec!["1".to_string(), "2".to_string()],
            token_uris: vec![
                "https://moonphase.is/image.svg".to_string(),
                "https://moonphase.is/image.svg".to_string(),
            ],
            receiver: "ekez".to_string(),
        }),
        &[],
    )
    .unwrap();

    // Get the address of the instantiated NFT.
    let nft: Addr = app
        .wrap()
        .query_wasm_smart(
            bridge.clone(),
            &QueryMsg::NftContractForClassId {
                class_id: "bad kids".to_string(),
            },
        )
        .unwrap();

    // Check that token_uri was set properly.
    let token_info: cw721::NftInfoResponse<Empty> = app
        .wrap()
        .query_wasm_smart(
            nft.clone(),
            &cw721::Cw721QueryMsg::NftInfo {
                token_id: "1".to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        token_info.token_uri,
        Some("https://moonphase.is/image.svg".to_string())
    );
    let token_info: cw721::NftInfoResponse<Empty> = app
        .wrap()
        .query_wasm_smart(
            nft.clone(),
            &cw721::Cw721QueryMsg::NftInfo {
                token_id: "2".to_string(),
            },
        )
        .unwrap();

    assert_eq!(
        token_info.token_uri,
        Some("https://moonphase.is/image.svg".to_string())
    );

    // Check that we can transfer the NFT via the ICS721 interface.
    app.execute_contract(
        Addr::unchecked("ekez"),
        nft.clone(),
        &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
            recipient: nft.to_string(),
            token_id: "1".to_string(),
        },
        &[],
    )
    .unwrap();

    let owner: cw721::OwnerOfResponse = app
        .wrap()
        .query_wasm_smart(
            bridge,
            &QueryMsg::Owner {
                token_id: "1".to_string(),
                class_id: "bad kids".to_string(),
            },
        )
        .unwrap();

    assert_eq!(owner.owner, nft.to_string());

    // Check that this state matches the state of the underlying
    // cw721.
    let base_owner: cw721::OwnerOfResponse = app
        .wrap()
        .query_wasm_smart(
            nft,
            &cw721::Cw721QueryMsg::OwnerOf {
                token_id: "1".to_string(),
                include_expired: None,
            },
        )
        .unwrap();

    assert_eq!(base_owner, owner);
}

#[test]
fn test_do_instantiate_and_mint_no_instantiate() {
    let mut app = App::default();

    let bridge = instantiate_bridge(&mut app);

    // This will instantiate a new contract for the class ID and then
    // do a mint.
    app.execute_contract(
        bridge.clone(),
        bridge.clone(),
        &ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
            class_id: "bad kids".to_string(),
            class_uri: Some("https://moonphase.is".to_string()),
            token_ids: vec!["1".to_string()],
            token_uris: vec!["https://moonphase.is/image.svg".to_string()],
            receiver: "ekez".to_string(),
        }),
        &[],
    )
    .unwrap();

    // This will only do a mint as the contract for the class ID has
    // already been instantiated.
    app.execute_contract(
        bridge.clone(),
        bridge.clone(),
        &ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
            class_id: "bad kids".to_string(),
            class_uri: Some("https://moonphase.is".to_string()),
            token_ids: vec!["2".to_string()],
            token_uris: vec!["https://moonphase.is/image.svg".to_string()],
            receiver: "ekez".to_string(),
        }),
        &[],
    )
    .unwrap();

    // Get the address of the instantiated NFT.
    let nft: Addr = app
        .wrap()
        .query_wasm_smart(
            bridge,
            &QueryMsg::NftContractForClassId {
                class_id: "bad kids".to_string(),
            },
        )
        .unwrap();

    // Make sure we have our tokens.
    let tokens: cw721::TokensResponse = app
        .wrap()
        .query_wasm_smart(
            nft,
            &cw721::Cw721QueryMsg::AllTokens {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert_eq!(tokens.tokens, vec!["1".to_string(), "2".to_string()])
}

#[test]
fn test_do_instantiate_and_mint_permissions() {
    let mut app = App::default();

    let bridge = instantiate_bridge(&mut app);

    // Method is only callable by the contract itself.
    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("ekez"),
            bridge,
            &ExecuteMsg::Callback(CallbackMsg::DoInstantiateAndMint {
                class_id: "bad kids".to_string(),
                class_uri: Some("https://moonphase.is".to_string()),
                token_ids: vec!["1".to_string()],
                token_uris: vec!["https://moonphase.is/image.svg".to_string()],
                receiver: "ekez".to_string(),
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Unauthorized {});
}

/// Tests that we can not proxy NFTs if no proxy is configured.
#[test]
fn test_no_proxy_unauthorized() {
    let mut app = App::default();

    let bridge_no_proxy = instantiate_bridge(&mut app);

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("proxy"),
            bridge_no_proxy,
            &ExecuteMsg::ReceiveProxyNft {
                eyeball: "nft".to_string(),
                msg: cw721::Cw721ReceiveMsg {
                    sender: "ekez".to_string(),
                    token_id: "1".to_string(),
                    msg: to_binary("").unwrap(),
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Unauthorized {});
}

// Tests that the proxy can send NFTs via this contract. multi test
// doesn't support IBC messages and panics with "unsupported type"
// when you try to send one. If we're sending an IBC message this test
// has passed though.
#[test]
#[should_panic(expected = "Unsupported type")]
fn test_proxy_authorized() {
    let mut app = App::default();

    let bridge = instantiate_bridge_with_proxy(&mut app, Some("proxy".to_string()));

    let cw721_id = app.store_code(cw721_contract());
    let cw721 = app
        .instantiate_contract(
            cw721_id,
            Addr::unchecked("ekez"),
            &cw721_base::InstantiateMsg {
                name: "token".to_string(),
                symbol: "nonfungible".to_string(),
                minter: "ekez".to_string(),
            },
            &[],
            "label cw721",
            None,
        )
        .unwrap();
    app.execute_contract(
        Addr::unchecked("ekez"),
        cw721.clone(),
        &cw721_base::ExecuteMsg::<Empty, Empty>::Mint(cw721_base::MintMsg {
            token_id: "1".to_string(),
            owner: "ekez".to_string(),
            token_uri: None,
            extension: Empty::default(),
        }),
        &[],
    )
    .unwrap();

    app.execute_contract(
        Addr::unchecked("proxy"),
        bridge,
        &ExecuteMsg::ReceiveProxyNft {
            eyeball: cw721.into_string(),
            msg: cw721::Cw721ReceiveMsg {
                sender: "ekez".to_string(),
                token_id: "1".to_string(),
                msg: to_binary(&IbcAwayMsg {
                    receiver: "ekez".to_string(),
                    channel_id: "channel-0".to_string(),
                    timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                        revision: 0,
                        height: 10,
                    }),
                })
                .unwrap(),
            },
        },
        &[],
    )
    .unwrap();
}

/// Tests that receiving a NFT via a regular receive fails when a
/// proxy is installed.
#[test]
fn test_no_receive_with_proxy() {
    let mut app = App::default();
    let bridge = instantiate_bridge_with_proxy(&mut app, Some("proxy".to_string()));

    let err: ContractError = app
        .execute_contract(
            Addr::unchecked("cw721"),
            bridge,
            &ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
                sender: "ekez".to_string(),
                token_id: "1".to_string(),
                msg: to_binary(&IbcAwayMsg {
                    receiver: "ekez".to_string(),
                    channel_id: "channel-0".to_string(),
                    timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                        revision: 0,
                        height: 10,
                    }),
                })
                .unwrap(),
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Unauthorized {})
}
