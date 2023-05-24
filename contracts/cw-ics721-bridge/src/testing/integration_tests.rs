use crate::{
    msg::{CallbackMsg, ExecuteMsg, IbcOutgoingMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    token_types::{Class, ClassId, Token, TokenId, VoucherCreation},
    ContractError,
};
use cosmwasm_std::{to_binary, Addr, Empty, IbcTimeout, IbcTimeoutBlock, WasmMsg};
use cw_cii::{Admin, ContractInstantiateInfo};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};
use cw_pause_once::PauseError;

const COMMUNITY_POOL: &str = "community_pool";

pub struct Test {
    pub app: App,
    pub cw721_id: u64,
    pub bridge_id: u64,
    pub bridge: Addr,
    pub proxy: Option<ContractInstantiateInfo>,
    pub pauser: Option<String>,
}

impl Test {
    pub fn instantiate_bridge(proxy: bool, pauser: Option<String>) -> Self {
        let mut app = App::default();
        let cw721_id = app.store_code(cw721_contract());
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
            bridge_id,
            bridge,
            proxy,
            pauser,
        }
    }

    pub fn pause_bridge(&mut self, sender: &str) {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.bridge.clone(),
                &ExecuteMsg::Pause {},
                &[],
            )
            .unwrap();
    }

    pub fn pause_bridge_should_fail(&mut self, sender: &str) -> ContractError {
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

    pub fn query_pause_info(&mut self) -> (bool, Option<Addr>) {
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

    pub fn query_cw721_id(&mut self) -> u64 {
        self.app
            .wrap()
            .query_wasm_smart(self.bridge.clone(), &QueryMsg::Cw721CodeId {})
            .unwrap()
    }

    pub fn query_nft_contracts(&mut self) -> Vec<(String, Addr)> {
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

    pub fn query_outgoing_channels(&mut self) -> Vec<((String, String), String)> {
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

    pub fn query_incoming_channels(&mut self) -> Vec<((String, String), String)> {
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
    .with_migrate(crate::contract::migrate)
    .with_reply(crate::ibc::reply);
    Box::new(contract)
}

fn proxy_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_rate_limited_proxy::contract::execute,
        cw721_rate_limited_proxy::contract::instantiate,
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
                receiver: "ekez".to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: None,
                        data: None,
                    },
                    tokens: vec![Token {
                        id: TokenId::new("1"),
                        // Empty URI string allowed.
                        uri: Some("".to_string()),
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
                receiver: "ekez".to_string(),
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
            Addr::unchecked("ekez"),
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
                receiver: "ekez".to_string(),
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
                receiver: "ekez".to_string(),
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
                receiver: "ekez".to_string(),
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

    let cw721_id = test.app.store_code(cw721_contract());
    let cw721 = test
        .app
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
    test.app
        .execute_contract(
            Addr::unchecked("ekez"),
            cw721.clone(),
            &cw721_base::ExecuteMsg::<Empty, Empty>::Mint(cw721_base::MintMsg {
                token_id: "1".to_string(),
                owner: test.bridge.to_string(),
                token_uri: None,
                extension: Empty::default(),
            }),
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
                    sender: "ekez".to_string(),
                    token_id: "1".to_string(),
                    msg: to_binary(&IbcOutgoingMsg {
                        receiver: "ekez".to_string(),
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
                sender: "ekez".to_string(),
                token_id: "1".to_string(),
                msg: to_binary(&IbcOutgoingMsg {
                    receiver: "ekez".to_string(),
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
    let mut test = Test::instantiate_bridge(true, Some("ekez".to_string()));

    // Should start unpaused.
    let (paused, pauser) = test.query_pause_info();
    assert!(!paused);
    assert_eq!(pauser, Some(Addr::unchecked("ekez")));

    // Non-pauser may not pause.
    let err = test.pause_bridge_should_fail("zeke");
    assert_eq!(
        err,
        ContractError::Pause(PauseError::Unauthorized {
            sender: Addr::unchecked("zeke")
        })
    );

    // Pause the bridge.
    test.pause_bridge("ekez");
    // Pausing should remove the pauser.
    let (paused, pauser) = test.query_pause_info();
    assert!(paused);
    assert_eq!(pauser, None);

    // Pausing fails.
    let err = test.pause_bridge_should_fail("ekez");
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
            Addr::unchecked("ekez"),
            WasmMsg::Migrate {
                contract_addr: test.bridge.to_string(),
                new_code_id: bridge_id,
                msg: to_binary(&MigrateMsg::WithUpdate {
                    pauser: Some("zeke".to_string()),
                    proxy: None,
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
