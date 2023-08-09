use cosmwasm_std::{
    testing::mock_env, to_binary, Addr, Binary, Deps, DepsMut, Empty, Env, IbcTimeout,
    IbcTimeoutBlock, MessageInfo, Reply, StdResult, WasmMsg,
};
use cw2::set_contract_version;
use cw721::ContractInfoResponse;
use cw721_base::msg::QueryMsg as Cw721QueryMsg;

use cw_cii::{Admin, ContractInstantiateInfo};
use cw_multi_test::{Contract, ContractWrapper, Executor};
use cw_pause_once::PauseError;
use ics721::{
    execute::Ics721Execute,
    ibc::Ics721Ibc,
    msg::{CallbackMsg, ExecuteMsg, IbcOutgoingMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    query::Ics721Query,
    token_types::{Class, ClassId, Token, TokenId, VoucherCreation},
};
use sg721_base::msg::{CollectionInfoResponse, QueryMsg as Sg721QueryMsg};
use sg_multi_test::StargazeApp;
use sg_std::{Response, StargazeMsgWrapper};

use crate::{ContractError, SgIcs721Contract};

const COMMUNITY_POOL: &str = "community_pool";
const CONTRACT_NAME: &str = "crates.io:cw-ics721-bridge";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    SgIcs721Contract::default().instantiate(deps, env, info, msg)
}

fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    SgIcs721Contract::default().execute(deps, env, info, msg)
}

fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    SgIcs721Contract::default().query(deps, env, msg)
}

fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    SgIcs721Contract::default().migrate(deps, env, msg)
}

struct Test {
    app: StargazeApp,
    cw721_id: u64,
    bridge: Addr,
    bridge_id: u64,
}

impl Test {
    fn instantiate_bridge(proxy: bool, pauser: Option<String>) -> Self {
        let mut app = StargazeApp::default();
        let cw721_id = app.store_code(sg721_base_contract());
        let bridge_id = app.store_code(bridge_contract());

        use cw721_rate_limited_proxy as rlp;
        let proxy = match proxy {
            true => {
                let proxy_id = app.store_code(proxy_contract());
                Some(ContractInstantiateInfo {
                    code_id: proxy_id,
                    msg: to_binary(&rlp::msg::InstantiateMsg {
                        rate_limit: rlp::Rate::PerBlock(10),
                        origin: None,
                    })
                    .unwrap(),
                    admin: Some(Admin::Instantiator {}),
                    label: "rate limited proxy".to_string(),
                })
            }
            false => None,
        };

        let bridge = app
            .instantiate_contract(
                bridge_id,
                Addr::unchecked(COMMUNITY_POOL),
                &InstantiateMsg {
                    cw721_base_code_id: cw721_id,
                    proxy: proxy.clone(),
                    pauser: pauser.clone(),
                },
                &[],
                "cw-ics721-bridge",
                pauser.clone(),
            )
            .unwrap();

        Self {
            app,
            cw721_id,
            bridge,
            bridge_id,
        }
    }

    fn pause_bridge(&mut self, sender: &str) {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.bridge.clone(),
                &ExecuteMsg::Pause {},
                &[],
            )
            .unwrap();
    }

    fn pause_bridge_should_fail(&mut self, sender: &str) -> ContractError {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.bridge.clone(),
                &ExecuteMsg::Pause {},
                &[],
            )
            .unwrap_err()
            .downcast()
            .unwrap()
    }

    fn query_pause_info(&mut self) -> (bool, Option<Addr>) {
        let paused = self
            .app
            .wrap()
            .query_wasm_smart(self.bridge.clone(), &QueryMsg::Paused {})
            .unwrap();
        let pauser = self
            .app
            .wrap()
            .query_wasm_smart(self.bridge.clone(), &QueryMsg::Pauser {})
            .unwrap();
        (paused, pauser)
    }

    fn query_proxy(&mut self) -> Option<Addr> {
        self.app
            .wrap()
            .query_wasm_smart(self.bridge.clone(), &QueryMsg::Proxy {})
            .unwrap()
    }

    fn query_cw721_id(&mut self) -> u64 {
        self.app
            .wrap()
            .query_wasm_smart(self.bridge.clone(), &QueryMsg::Cw721CodeId {})
            .unwrap()
    }

    fn query_nft_contracts(&mut self) -> Vec<(String, Addr)> {
        self.app
            .wrap()
            .query_wasm_smart(
                self.bridge.clone(),
                &QueryMsg::NftContracts {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
    }

    fn query_outgoing_channels(&mut self) -> Vec<((String, String), String)> {
        self.app
            .wrap()
            .query_wasm_smart(
                self.bridge.clone(),
                &QueryMsg::OutgoingChannels {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
    }

    fn query_incoming_channels(&mut self) -> Vec<((String, String), String)> {
        self.app
            .wrap()
            .query_wasm_smart(
                self.bridge.clone(),
                &QueryMsg::OutgoingChannels {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
    }
}

fn sg721_base_contract() -> Box<dyn Contract<StargazeMsgWrapper>> {
    let contract = ContractWrapper::new(
        sg721_base::entry::execute,
        sg721_base::entry::instantiate,
        sg721_base::entry::query,
    );
    Box::new(contract)
}

fn bridge_contract() -> Box<dyn Contract<StargazeMsgWrapper>> {
    // need to wrap method in function for testing
    fn ibc_reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
        SgIcs721Contract::default()
            .reply(deps, env, reply)
            .and_then(|_| Ok(Response::default()))
    }

    let contract = ContractWrapper::new(execute, instantiate, query)
        .with_migrate(migrate)
        .with_reply(ibc_reply);
    Box::new(contract)
}

fn proxy_contract() -> Box<dyn Contract<StargazeMsgWrapper>> {
    let execute_fn = cw721_rate_limited_proxy::contract::execute::<StargazeMsgWrapper>;
    let instatiate_fn = cw721_rate_limited_proxy::contract::instantiate::<StargazeMsgWrapper>;
    let contract = ContractWrapper::new(
        execute_fn,
        instatiate_fn,
        cw721_rate_limited_proxy::contract::query,
    );
    Box::new(contract)
}

#[test]
fn test_instantiate() {
    let mut test = Test::instantiate_bridge(false, None);

    // check stores are properly initialized
    let cw721_id = test.query_cw721_id();
    assert_eq!(cw721_id, test.cw721_id);
    let nft_contracts: Vec<(String, Addr)> = test.query_nft_contracts();
    assert_eq!(nft_contracts, Vec::<(String, Addr)>::new());
    let outgoing_channels = test.query_outgoing_channels();
    assert_eq!(outgoing_channels, []);
    let incoming_channels = test.query_incoming_channels();
    assert_eq!(incoming_channels, []);
}

#[test]
fn test_do_instantiate_and_mint_weird_data() {
    let mut test = Test::instantiate_bridge(false, None);

    test.app
        .execute_contract(
            test.bridge.clone(),
            test.bridge,
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: "mr-t".to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: None,
                        data: None,
                    },
                    tokens: vec![Token {
                        id: TokenId::new("1"),
                        // IMPORTANT: unlike cw721-base, for sg721-base empty URI string is NOT allowed
                        uri: Some("arkprotocol".to_string()),
                        data: None,
                    }],
                },
            }),
            &[],
        )
        .unwrap();
}

#[test]
fn test_do_instantiate_and_mint() {
    let mut test = Test::instantiate_bridge(false, None);

    test.app
        .execute_contract(
            test.bridge.clone(),
            test.bridge.clone(),
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: "mr-t".to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: Some("https://moonphase.is".to_string()),
                        data: None,
                    },
                    tokens: vec![
                        Token {
                            id: TokenId::new("1"),
                            uri: Some("https://moonphase.is/image.svg".to_string()),
                            data: None,
                        },
                        Token {
                            id: TokenId::new("2"),
                            uri: Some("https://moonphase.is/image.svg".to_string()),
                            data: None,
                        },
                    ],
                },
            }),
            &[],
        )
        .unwrap();
    // Check entry added in CLASS_ID_TO_NFT_CONTRACT
    let nft_contracts = test.query_nft_contracts();
    assert_eq!(nft_contracts.len(), 1);
    assert_eq!(nft_contracts[0].0, "bad kids");
    // Get the address of the instantiated NFT.
    let nft: Addr = test
        .app
        .wrap()
        .query_wasm_smart(
            test.bridge.clone(),
            &QueryMsg::NftContract {
                class_id: "bad kids".to_string(),
            },
        )
        .unwrap();

    // check contract info is properly set
    let contract_info: ContractInfoResponse = test
        .app
        .wrap()
        .query_wasm_smart(nft.clone(), &Cw721QueryMsg::<Empty>::ContractInfo {})
        .unwrap();
    assert_eq!(
        contract_info,
        ContractInfoResponse {
            name: "bad kids".to_string(),
            symbol: "bad kids".to_string()
        }
    );

    // check collection info is properly set
    let collection_info: CollectionInfoResponse = test
        .app
        .wrap()
        .query_wasm_smart(nft.clone(), &Sg721QueryMsg::CollectionInfo {})
        .unwrap();
    assert_eq!(
        collection_info,
        CollectionInfoResponse {
            creator: test.bridge.to_string(),
            description: "".to_string(),
            image: "https://arkprotocol.io".to_string(),
            external_link: None,
            explicit_content: None,
            start_trading_time: None,
            royalty_info: None,
        }
    );

    // Check that token_uri was set properly.
    let token_info: cw721::NftInfoResponse<Empty> = test
        .app
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
    let token_info: cw721::NftInfoResponse<Empty> = test
        .app
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
    test.app
        .execute_contract(
            Addr::unchecked("mr-t"),
            nft.clone(),
            &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
                recipient: nft.to_string(),
                token_id: "1".to_string(),
            },
            &[],
        )
        .unwrap();

    let owner: cw721::OwnerOfResponse = test
        .app
        .wrap()
        .query_wasm_smart(
            test.bridge,
            &QueryMsg::Owner {
                token_id: "1".to_string(),
                class_id: "bad kids".to_string(),
            },
        )
        .unwrap();

    assert_eq!(owner.owner, nft.to_string());

    // Check that this state matches the state of the underlying
    // cw721.
    let base_owner: cw721::OwnerOfResponse = test
        .app
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
    let mut test = Test::instantiate_bridge(false, None);

    // This will instantiate a new contract for the class ID and then
    // do a mint.
    test.app
        .execute_contract(
            test.bridge.clone(),
            test.bridge.clone(),
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: "mr-t".to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: Some("https://moonphase.is".to_string()),
                        data: None,
                    },
                    tokens: vec![Token {
                        id: TokenId::new("1"),
                        uri: Some("https://moonphase.is/image.svg".to_string()),
                        data: None,
                    }],
                },
            }),
            &[],
        )
        .unwrap();

    // Check entry added in CLASS_ID_TO_NFT_CONTRACT
    let class_id_to_nft_contract = test.query_nft_contracts();
    assert_eq!(class_id_to_nft_contract.len(), 1);
    assert_eq!(class_id_to_nft_contract[0].0, "bad kids");

    // This will only do a mint as the contract for the class ID has
    // already been instantiated.
    test.app
        .execute_contract(
            test.bridge.clone(),
            test.bridge.clone(),
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: "mr-t".to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: Some("https://moonphase.is".to_string()),
                        data: None,
                    },
                    tokens: vec![Token {
                        id: TokenId::new("2"),
                        uri: Some("https://moonphase.is/image.svg".to_string()),
                        data: None,
                    }],
                },
            }),
            &[],
        )
        .unwrap();

    // Check no additional entry added in CLASS_ID_TO_NFT_CONTRACT
    let class_id_to_nft_contract = test.query_nft_contracts();
    assert_eq!(class_id_to_nft_contract.len(), 1);

    // Get the address of the instantiated NFT.
    let nft: Addr = test
        .app
        .wrap()
        .query_wasm_smart(
            test.bridge,
            &QueryMsg::NftContract {
                class_id: "bad kids".to_string(),
            },
        )
        .unwrap();

    // Make sure we have our tokens.
    let tokens: cw721::TokensResponse = test
        .app
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
    let mut test = Test::instantiate_bridge(false, None);

    // Method is only callable by the contract itself.
    let err: ContractError = test
        .app
        .execute_contract(
            Addr::unchecked("notbridge"),
            test.bridge,
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: "mr-t".to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: Some("https://moonphase.is".to_string()),
                        data: None,
                    },
                    tokens: vec![Token {
                        id: TokenId::new("1"),
                        uri: Some("https://moonphase.is/image.svg".to_string()),
                        data: None,
                    }],
                },
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
    let mut test = Test::instantiate_bridge(false, None);

    let err: ContractError = test
        .app
        .execute_contract(
            Addr::unchecked("proxy"),
            test.bridge,
            &ExecuteMsg::ReceiveProxyNft {
                eyeball: "nft".to_string(),
                msg: cw721::Cw721ReceiveMsg {
                    sender: "mr-t".to_string(),
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
// doesn't support IBC messages and panics with "Unexpected exec msg
// SendPacket" when you try to send one. If we're sending an IBC
// message this test has passed though.
//
// NOTE: this test may fail when updating multi-test as the panic
// string may change.
#[test]
#[should_panic(expected = "Unexpected exec msg SendPacket")]
fn test_proxy_authorized() {
    let mut test = Test::instantiate_bridge(true, None);

    let proxy_address: Option<Addr> = test
        .app
        .wrap()
        .query_wasm_smart(&test.bridge, &QueryMsg::Proxy {})
        .unwrap();
    let proxy_address = proxy_address.expect("expected a proxy");

    let cw721_id = test.app.store_code(sg721_base_contract());
    let cw721 = test
        .app
        .instantiate_contract(
            cw721_id,
            test.bridge.clone(), // sg721 contract can only be instantiated by a contract, not user (unauthorized)
            &sg721::InstantiateMsg {
                name: "token".to_string(),
                symbol: "nonfungible".to_string(),
                minter: "mr-t".to_string(),
                collection_info: sg721::CollectionInfo {
                    creator: mock_env().contract.address.to_string(),
                    description: "".to_string(),
                    image: "https://arkprotocol.io".to_string(),
                    external_link: None,
                    explicit_content: None,
                    start_trading_time: None,
                    royalty_info: None,
                },
            },
            &[],
            "label cw721",
            None,
        )
        .unwrap();
    test.app
        .execute_contract(
            Addr::unchecked("mr-t"),
            cw721.clone(),
            &cw721_base::ExecuteMsg::<Empty, Empty>::Mint {
                token_id: "1".to_string(),
                owner: test.bridge.to_string(),
                token_uri: None,
                extension: Empty::default(),
            },
            &[],
        )
        .unwrap();

    test.app
        .execute_contract(
            proxy_address,
            test.bridge,
            &ExecuteMsg::ReceiveProxyNft {
                eyeball: cw721.into_string(),
                msg: cw721::Cw721ReceiveMsg {
                    sender: "mr-t".to_string(),
                    token_id: "1".to_string(),
                    msg: to_binary(&IbcOutgoingMsg {
                        receiver: "mr-t".to_string(),
                        channel_id: "channel-0".to_string(),
                        timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                            revision: 0,
                            height: 10,
                        }),
                        memo: None,
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
    let mut test = Test::instantiate_bridge(true, None);

    let err: ContractError = test
        .app
        .execute_contract(
            Addr::unchecked("cw721"),
            test.bridge,
            &ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
                sender: "mr-t".to_string(),
                token_id: "1".to_string(),
                msg: to_binary(&IbcOutgoingMsg {
                    receiver: "mr-t".to_string(),
                    channel_id: "channel-0".to_string(),
                    timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                        revision: 0,
                        height: 10,
                    }),
                    memo: None,
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

/// Tests the contract's pause behavior.
#[test]
fn test_pause() {
    let mut test = Test::instantiate_bridge(true, Some("mr-t".to_string()));

    // Should start unpaused.
    let (paused, pauser) = test.query_pause_info();
    assert!(!paused);
    assert_eq!(pauser, Some(Addr::unchecked("mr-t")));

    // Non-pauser may not pause.
    let err = test.pause_bridge_should_fail("zeke");
    assert_eq!(
        err,
        ContractError::Pause(PauseError::Unauthorized {
            sender: Addr::unchecked("zeke")
        })
    );

    // Pause the bridge.
    test.pause_bridge("mr-t");
    // Pausing should remove the pauser.
    let (paused, pauser) = test.query_pause_info();
    assert!(paused);
    assert_eq!(pauser, None);

    // Pausing fails.
    let err = test.pause_bridge_should_fail("mr-t");
    assert_eq!(err, ContractError::Pause(PauseError::Paused {}));

    // Even something like executing a callback on ourselves will be
    // caught by a pause.
    let err: ContractError = test
        .app
        .execute_contract(
            test.bridge.clone(),
            test.bridge.clone(),
            &ExecuteMsg::Callback(CallbackMsg::Conjunction { operands: vec![] }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Pause(PauseError::Paused {}));

    // Set a new pauser.
    let bridge_id = test.app.store_code(bridge_contract());
    test.app
        .execute(
            Addr::unchecked("mr-t"),
            WasmMsg::Migrate {
                contract_addr: test.bridge.to_string(),
                new_code_id: bridge_id,
                msg: to_binary(&MigrateMsg::WithUpdate {
                    pauser: Some("zeke".to_string()),
                    proxy: None,
                    cw721_base_code_id: None,
                })
                .unwrap(),
            }
            .into(),
        )
        .unwrap();

    // Setting new pauser should unpause.
    let (paused, pauser) = test.query_pause_info();
    assert!(!paused);
    assert_eq!(pauser, Some(Addr::unchecked("zeke")));

    // One more pause for posterity sake.
    test.pause_bridge("zeke");
    let (paused, pauser) = test.query_pause_info();
    assert!(paused);
    assert_eq!(pauser, None);
}

/// Tests migration.
#[test]
fn test_migration() {
    let mut test = Test::instantiate_bridge(true, Some("mr-t".to_string()));
    // assert instantiation worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(pauser, Some(Addr::unchecked("mr-t")));
    let proxy = test.query_proxy();
    assert!(proxy.is_some());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, test.cw721_id);

    // migrate changes
    test.app
        .execute(
            Addr::unchecked("mr-t"),
            WasmMsg::Migrate {
                contract_addr: test.bridge.to_string(),
                new_code_id: test.bridge_id,
                msg: to_binary(&MigrateMsg::WithUpdate {
                    pauser: None,
                    proxy: None,
                    cw721_base_code_id: Some(12345678),
                })
                .unwrap(),
            }
            .into(),
        )
        .unwrap();
    // assert migration worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(pauser, None);
    let proxy = test.query_proxy();
    assert!(proxy.is_none());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, 12345678);

    // migrate without changing code id
    test.app
        .execute(
            Addr::unchecked("mr-t"),
            WasmMsg::Migrate {
                contract_addr: test.bridge.to_string(),
                new_code_id: test.bridge_id,
                msg: to_binary(&MigrateMsg::WithUpdate {
                    pauser: None,
                    proxy: None,
                    cw721_base_code_id: None,
                })
                .unwrap(),
            }
            .into(),
        )
        .unwrap();
    // assert migration worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(pauser, None);
    let proxy = test.query_proxy();
    assert!(proxy.is_none());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, 12345678);
}
