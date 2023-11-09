use anyhow::Result;
use bech32::{decode, encode, FromBase32, ToBase32, Variant};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_binary, from_json, instantiate2_address, to_binary, to_json_binary, Addr, Api, Binary,
    CanonicalAddr, Deps, DepsMut, Empty, Env, GovMsg, IbcTimeout, IbcTimeoutBlock, MemoryStorage,
    MessageInfo, RecoverPubkeyError, Reply, Response, StdError, StdResult, Storage,
    VerificationError, WasmMsg,
};
use cw2::set_contract_version;
use cw721_base::msg::{InstantiateMsg as Cw721InstantiateMsg, QueryMsg as Cw721QueryMsg};
use cw_cii::{Admin, ContractInstantiateInfo};
use cw_multi_test::{
    AddressGenerator, App, AppBuilder, BankKeeper, Contract, ContractWrapper, DistributionKeeper,
    Executor, FailingModule, IbcAcceptingModule, Router, StakeKeeper, WasmKeeper,
};
use cw_pause_once::PauseError;
use sha2::{digest::Update, Digest, Sha256};

use crate::{
    execute::Ics721Execute,
    ibc::Ics721Ibc,
    msg::{CallbackMsg, ExecuteMsg, IbcOutgoingMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    query::Ics721Query,
    state::CollectionData,
    token_types::{Class, ClassId, Token, TokenId, VoucherCreation},
    ContractError,
};

use super::contract::Ics721Contract;

const ICS721_CREATOR: &str = "ics721-creator";
const CONTRACT_NAME: &str = "crates.io:ics721-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

const OWNER_SOURCE_CHAIN: &str = "juno1ke55z7catvdvnhvyyh0pkvs30t09me72vcxkh5";
const SOURCE_HRP: &str = "juno";

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

fn instantiate(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;
    Ics721Contract::default().instantiate(deps, env, info, msg)
}

fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    Ics721Contract::default().execute(deps, env, info, msg)
}

fn query(deps: Deps, env: Env, msg: QueryMsg) -> StdResult<Binary> {
    Ics721Contract::default().query(deps, env, msg)
}

fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    Ics721Contract::default().migrate(deps, env, msg)
}

fn no_init(
    _router: &mut Router<
        BankKeeper,
        FailingModule<Empty, Empty, Empty>,
        WasmKeeper<Empty, Empty>,
        StakeKeeper,
        DistributionKeeper,
        IbcAcceptingModule,
        FailingModule<GovMsg, Empty, Empty>,
    >,
    _api: &dyn Api,
    _storage: &mut dyn Storage,
) {
}

#[derive(Default)]
pub struct MockAddressGenerator;

impl AddressGenerator for MockAddressGenerator {
    fn contract_address(
        &self,
        api: &dyn Api,
        _storage: &mut dyn Storage,
        code_id: u64,
        instance_id: u64,
    ) -> Result<Addr> {
        let canonical_addr = Self::instantiate_address(code_id, instance_id);
        Ok(Addr::unchecked(api.addr_humanize(&canonical_addr)?))
    }

    fn predictable_contract_address(
        &self,
        api: &dyn Api,
        _storage: &mut dyn Storage,
        _code_id: u64,
        _instance_id: u64,
        checksum: &[u8],
        creator: &CanonicalAddr,
        salt: &[u8],
    ) -> Result<Addr> {
        // string representation of the salt
        let salt_to_string = std::str::from_utf8(salt)?;
        // Remove the square brackets and split the string by commas
        let parts: Vec<&str> = salt_to_string
            .trim_matches(|c| c == '[' || c == ']')
            .split(',')
            .collect();
        // Convert each part to a u8 and collect them into a Vec<u8>
        let salt: Vec<u8> = parts
            .iter()
            .map(|part| part.trim().parse().unwrap())
            .collect();
        let canonical_addr = instantiate2_address(checksum, creator, &salt)?;
        Ok(Addr::unchecked(api.addr_humanize(&canonical_addr)?))
    }
}

impl MockAddressGenerator {
    // non-predictable contract address generator, see `BuildContractAddressClassic`
    // implementation in wasmd: https://github.com/CosmWasm/wasmd/blob/main/x/wasm/keeper/addresses.go#L35-L42
    fn instantiate_address(code_id: u64, instance_id: u64) -> CanonicalAddr {
        let mut key = Vec::<u8>::new();
        key.extend_from_slice(b"wasm\0");
        key.extend_from_slice(&code_id.to_be_bytes());
        key.extend_from_slice(&instance_id.to_be_bytes());
        let module = Sha256::digest("module".as_bytes());
        let result = Sha256::new()
            .chain(module)
            .chain(key)
            .finalize()
            .to_vec()
            .into();
        return result;
    }
}
pub struct MockApiBech32 {
    prefix: &'static str,
}

impl MockApiBech32 {
    pub fn new(prefix: &'static str) -> Self {
        Self { prefix }
    }
}

impl Api for MockApiBech32 {
    fn addr_validate(&self, input: &str) -> StdResult<Addr> {
        let canonical = self.addr_canonicalize(input)?;
        let normalized = self.addr_humanize(&canonical)?;
        if input != normalized {
            Err(StdError::generic_err(
                "Invalid input: address not normalized",
            ))
        } else {
            Ok(Addr::unchecked(input))
        }
    }

    fn addr_canonicalize(&self, input: &str) -> StdResult<CanonicalAddr> {
        if let Ok((prefix, decoded, Variant::Bech32)) = decode(input) {
            if prefix == self.prefix {
                if let Ok(bytes) = Vec::<u8>::from_base32(&decoded) {
                    return Ok(bytes.into());
                }
            }
        }
        Err(StdError::generic_err(format!("Invalid input: {}", input)))
    }

    fn addr_humanize(&self, canonical: &CanonicalAddr) -> StdResult<Addr> {
        if let Ok(encoded) = encode(
            self.prefix,
            canonical.as_slice().to_base32(),
            Variant::Bech32,
        ) {
            Ok(Addr::unchecked(encoded))
        } else {
            Err(StdError::generic_err("Invalid canonical address"))
        }
    }

    fn secp256k1_verify(
        &self,
        _message_hash: &[u8],
        _signature: &[u8],
        _public_key: &[u8],
    ) -> Result<bool, VerificationError> {
        unimplemented!()
    }

    fn secp256k1_recover_pubkey(
        &self,
        _message_hash: &[u8],
        _signature: &[u8],
        _recovery_param: u8,
    ) -> Result<Vec<u8>, RecoverPubkeyError> {
        unimplemented!()
    }

    fn ed25519_verify(
        &self,
        _message: &[u8],
        _signature: &[u8],
        _public_key: &[u8],
    ) -> Result<bool, VerificationError> {
        unimplemented!()
    }

    fn ed25519_batch_verify(
        &self,
        _messages: &[&[u8]],
        _signatures: &[&[u8]],
        _public_keys: &[&[u8]],
    ) -> Result<bool, VerificationError> {
        unimplemented!()
    }

    fn debug(&self, _message: &str) {
        unimplemented!()
    }
}

impl MockApiBech32 {
    pub fn addr_make(&self, input: &str) -> Addr {
        let digest = Sha256::digest(input).to_vec();
        match encode(self.prefix, digest.to_base32(), Variant::Bech32) {
            Ok(address) => Addr::unchecked(address),
            Err(reason) => panic!("Generating address failed with reason: {reason}"),
        }
    }
}

#[cw_serde]
pub struct CustomClassData {
    pub foo: Option<String>,
}

struct Test {
    app: App<
        BankKeeper,
        MockApiBech32,
        MemoryStorage,
        FailingModule<Empty, Empty, Empty>,
        WasmKeeper<Empty, Empty>,
        StakeKeeper,
        DistributionKeeper,
        IbcAcceptingModule,
    >,
    minter: Addr,
    cw721_id: u64,
    cw721: Addr,
    ics721_id: u64,
    ics721: Addr,
    nfts_minted: usize,
}

impl Test {
    fn new(proxy: bool, pauser: Option<String>, cw721_code: Box<dyn Contract<Empty>>) -> Self {
        let mut app = AppBuilder::new()
            .with_wasm::<WasmKeeper<Empty, Empty>>(
                WasmKeeper::new().with_address_generator(MockAddressGenerator),
            )
            .with_ibc(IbcAcceptingModule::default())
            .with_api(MockApiBech32::new(SOURCE_HRP))
            .build(no_init);
        let cw721_id = app.store_code(cw721_code);
        let ics721_id = app.store_code(ics721_contract());

        use cw721_rate_limited_proxy as rlp;
        let proxy = match proxy {
            true => {
                let proxy_id = app.store_code(proxy_contract());
                Some(ContractInstantiateInfo {
                    code_id: proxy_id,
                    msg: to_json_binary(&rlp::msg::InstantiateMsg {
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

        let ics721 = app
            .instantiate_contract(
                ics721_id,
                app.api().addr_make(ICS721_CREATOR),
                &InstantiateMsg {
                    cw721_base_code_id: cw721_id,
                    proxy: proxy.clone(),
                    pauser: pauser
                        .clone()
                        .and_then(|p| Some(app.api().addr_make(&p).to_string())),
                },
                &[],
                "ics721-base",
                pauser
                    .clone()
                    .and_then(|p| Some(app.api().addr_make(&p).to_string())),
            )
            .unwrap();

        let minter = app.api().addr_make("minter");
        let cw721 = app
            .instantiate_contract(
                cw721_id,
                minter.clone(),
                &Cw721InstantiateMsg {
                    name: "name".to_string(),
                    symbol: "symbol".to_string(),
                    minter: minter.to_string(),
                },
                &[],
                "cw721-base",
                None,
            )
            .unwrap();

        Self {
            app,
            minter,
            cw721_id,
            cw721,
            ics721_id,
            ics721,
            nfts_minted: 0,
        }
    }

    fn pause_ics721(&mut self, sender: &str) {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.ics721.clone(),
                &ExecuteMsg::Pause {},
                &[],
            )
            .unwrap();
    }

    fn pause_ics721_should_fail(&mut self, sender: &str) -> ContractError {
        self.app
            .execute_contract(
                Addr::unchecked(sender),
                self.ics721.clone(),
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
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::Paused {})
            .unwrap();
        let pauser = self
            .app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::Pauser {})
            .unwrap();
        (paused, pauser)
    }

    fn query_proxy(&mut self) -> Option<Addr> {
        self.app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::Proxy {})
            .unwrap()
    }

    fn query_cw721_id(&mut self) -> u64 {
        self.app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::Cw721CodeId {})
            .unwrap()
    }

    fn query_nft_contracts(&mut self) -> Vec<(String, Addr)> {
        self.app
            .wrap()
            .query_wasm_smart(
                self.ics721.clone(),
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
                self.ics721.clone(),
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
                self.ics721.clone(),
                &QueryMsg::OutgoingChannels {
                    start_after: None,
                    limit: None,
                },
            )
            .unwrap()
    }

    fn execute_cw721_mint(&mut self, owner: Addr) -> Result<String, anyhow::Error> {
        self.nfts_minted += 1;

        self.app
            .execute_contract(
                self.minter.clone(),
                self.cw721.clone(),
                &cw721_base::msg::ExecuteMsg::<Empty, Empty>::Mint {
                    token_id: self.nfts_minted.to_string(),
                    owner: owner.to_string(),
                    token_uri: None,
                    extension: Default::default(),
                },
                &[],
            )
            .map(|_| self.nfts_minted.to_string())
    }
}

fn cw721_base_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_base::entry::execute,
        cw721_base::entry::instantiate,
        cw721_base::entry::query,
    );
    Box::new(contract)
}

fn cw721_v016_base_contract() -> Box<dyn Contract<Empty>> {
    use cw721_base_016 as v016;
    let contract = ContractWrapper::new(
        v016::entry::execute,
        v016::entry::instantiate,
        v016::entry::query,
    );
    Box::new(contract)
}

fn ics721_contract() -> Box<dyn Contract<Empty>> {
    // need to wrap method in function for testing
    fn ibc_reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
        Ics721Contract::default().reply(deps, env, reply)
    }

    let contract = ContractWrapper::new(execute, instantiate, query)
        .with_migrate(migrate)
        .with_reply(ibc_reply);
    Box::new(contract)
}

fn proxy_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_rate_limited_proxy::contract::execute::<Empty>,
        cw721_rate_limited_proxy::contract::instantiate,
        cw721_rate_limited_proxy::contract::query,
    );
    Box::new(contract)
}

#[test]
fn test_instantiate() {
    let mut test = Test::new(false, None, cw721_base_contract());

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
    let mut test = Test::new(false, None, cw721_base_contract());

    test.app
        .execute_contract(
            test.ics721.clone(),
            test.ics721,
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make("ekez").to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: None,
                        data: Some(
                            to_json_binary(&CollectionData {
                                owner: Some(
                                    test.app.api().addr_make(OWNER_SOURCE_CHAIN).to_string(),
                                ),
                                contract_info: Default::default(),
                                name: "name".to_string(),
                                symbol: "symbol".to_string(),
                                num_tokens: 1,
                            })
                            .unwrap(),
                        ),
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
    // test case: instantiate cw721 with no ClassData
    {
        let mut test = Test::new(false, None, cw721_base_contract());
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make("ekez").to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new("bad kids"),
                            uri: Some("https://moonphase.is".to_string()),
                            data: None, // no class data
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
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: "bad kids".to_string(),
                },
            )
            .unwrap();

        // check contract info is properly set
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft.clone(), &Cw721QueryMsg::<Empty>::ContractInfo {})
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::ContractInfoResponse {
                name: "bad kids".to_string(),
                symbol: "bad kids".to_string()
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
                test.app.api().addr_make("ekez"),
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
                test.ics721,
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
    // test case: instantiate cw721 with ClassData containing owner
    {
        let mut test = Test::new(false, None, cw721_base_contract());
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make("ekez").to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new("bad kids"),
                            uri: Some("https://moonphase.is".to_string()),
                            data: Some(
                                to_json_binary(&CollectionData {
                                    owner: Some(
                                        test.app.api().addr_make(OWNER_SOURCE_CHAIN).to_string(),
                                    ),
                                    contract_info: Default::default(),
                                    name: "name".to_string(),
                                    symbol: "symbol".to_string(),
                                    num_tokens: 1,
                                })
                                .unwrap(),
                            ),
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
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: "bad kids".to_string(),
                },
            )
            .unwrap();

        // check contract info is properly set
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft.clone(), &Cw721QueryMsg::<Empty>::ContractInfo {})
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::ContractInfoResponse {
                name: "bad kids".to_string(),
                symbol: "bad kids".to_string()
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
                test.app.api().addr_make("ekez"),
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
                test.ics721,
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
    // test case: instantiate cw721 with different CustomClassData with no owner info
    {
        let mut test = Test::new(false, None, cw721_base_contract());
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make("ekez").to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new("bad kids"),
                            uri: Some("https://moonphase.is".to_string()),
                            data: Some(
                                to_json_binary(&CustomClassData {
                                    foo: Some(
                                        test.app.api().addr_make(OWNER_SOURCE_CHAIN).to_string(),
                                    ),
                                })
                                .unwrap(),
                            ),
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
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: "bad kids".to_string(),
                },
            )
            .unwrap();

        // check contract info is properly set
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft.clone(), &Cw721QueryMsg::<Empty>::ContractInfo {})
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::ContractInfoResponse {
                name: "bad kids".to_string(),
                symbol: "bad kids".to_string()
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
                test.app.api().addr_make("ekez"),
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
                test.ics721,
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
}

#[test]
fn test_do_instantiate_and_mint_no_instantiate() {
    let mut test = Test::new(false, None, cw721_base_contract());

    // This will instantiate a new contract for the class ID and then
    // do a mint.
    test.app
        .execute_contract(
            test.ics721.clone(),
            test.ics721.clone(),
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make("ekez").to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: Some("https://moonphase.is".to_string()),
                        data: Some(
                            to_json_binary(&CollectionData {
                                owner: Some(
                                    test.app.api().addr_make(OWNER_SOURCE_CHAIN).to_string(),
                                ),
                                contract_info: Default::default(),
                                name: "name".to_string(),
                                symbol: "symbol".to_string(),
                                num_tokens: 1,
                            })
                            .unwrap(),
                        ),
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
            test.ics721.clone(),
            test.ics721.clone(),
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make("ekez").to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: Some("https://moonphase.is".to_string()),
                        // unlike above in 1st transfer, here on 2nd transfer no classdata is provided!
                        // this won't affect collection since it's already instantiated
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
            test.ics721,
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
    let mut test = Test::new(false, None, cw721_base_contract());

    // Method is only callable by the contract itself.
    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("notIcs721"),
            test.ics721,
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make("ekez").to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new("bad kids"),
                        uri: Some("https://moonphase.is".to_string()),
                        data: Some(
                            to_json_binary(&CollectionData {
                                owner: Some(
                                    test.app.api().addr_make(OWNER_SOURCE_CHAIN).to_string(),
                                ),
                                contract_info: Default::default(),
                                name: "name".to_string(),
                                symbol: "symbol".to_string(),
                                num_tokens: 1,
                            })
                            .unwrap(),
                        ),
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
    let mut test = Test::new(false, None, cw721_base_contract());

    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("proxy"),
            test.ics721,
            &ExecuteMsg::ReceiveProxyNft {
                eyeball: "nft".to_string(),
                msg: cw721::Cw721ReceiveMsg {
                    sender: test.app.api().addr_make("ekez").to_string(),
                    token_id: "1".to_string(),
                    msg: to_json_binary("").unwrap(),
                },
            },
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();

    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_proxy_authorized() {
    let mut test = Test::new(true, None, cw721_base_contract());

    let proxy_address: Option<Addr> = test
        .app
        .wrap()
        .query_wasm_smart(&test.ics721, &QueryMsg::Proxy {})
        .unwrap();
    let proxy_address = proxy_address.expect("expected a proxy");

    let cw721_id = test.app.store_code(cw721_base_contract());
    let cw721 = test
        .app
        .instantiate_contract(
            cw721_id,
            test.app.api().addr_make("ekez"),
            &cw721_base::InstantiateMsg {
                name: "token".to_string(),
                symbol: "nonfungible".to_string(),
                minter: test.app.api().addr_make("ekez").to_string(),
            },
            &[],
            "label cw721",
            None,
        )
        .unwrap();
    test.app
        .execute_contract(
            test.app.api().addr_make("ekez"),
            cw721.clone(),
            &cw721_base::ExecuteMsg::<Empty, Empty>::Mint {
                token_id: "1".to_string(),
                owner: test.ics721.to_string(),
                token_uri: None,
                extension: Empty::default(),
            },
            &[],
        )
        .unwrap();

    test.app
        .execute_contract(
            proxy_address,
            test.ics721,
            &ExecuteMsg::ReceiveProxyNft {
                eyeball: cw721.into_string(),
                msg: cw721::Cw721ReceiveMsg {
                    sender: test.app.api().addr_make("ekez").to_string(),
                    token_id: "1".to_string(),
                    msg: to_json_binary(&IbcOutgoingMsg {
                        receiver: test.app.api().addr_make("ekez").to_string(),
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

#[test]
fn test_receive_nft() {
    // test case: receive nft from cw721-base
    {
        let mut test = Test::new(false, None, cw721_base_contract());
        // mint and escrowed/owned by ics721
        let token_id = test.execute_cw721_mint(test.ics721.clone()).unwrap();

        let res = test
            .app
            .execute_contract(
                test.cw721.clone(),
                test.ics721,
                &ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
                    sender: test.minter.to_string(),
                    token_id: token_id.clone(),
                    msg: to_json_binary(&IbcOutgoingMsg {
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
            .unwrap();
        let event = res.events.into_iter().find(|e| e.ty == "wasm").unwrap();
        let class_data_attribute = event
            .attributes
            .into_iter()
            .find(|a| a.key == "class_data")
            .unwrap();
        let expected_contract_info: cosmwasm_std::ContractInfoResponse = from_json(
            &to_json_binary(&ContractInfoResponse {
                code_id: test.cw721_id,
                creator: test.minter.to_string(),
                admin: None,
                pinned: false,
                ibc_port: None,
            })
            .unwrap(),
        )
        .unwrap();
        let expected_collection_data = to_json_binary(&CollectionData {
            owner: Some(test.minter.to_string()),
            contract_info: expected_contract_info,
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            num_tokens: 1,
        })
        .unwrap();
        assert_eq!(
            class_data_attribute.value,
            format!("{:?}", expected_collection_data)
        );
    }
    // test case: receive nft from old/v016 cw721-base
    {
        let mut test = Test::new(false, None, cw721_v016_base_contract());
        // mint and escrowed/owned by ics721
        let token_id = test.execute_cw721_mint(test.ics721.clone()).unwrap();

        let res = test
            .app
            .execute_contract(
                test.cw721.clone(),
                test.ics721,
                &ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
                    sender: test.minter.to_string(),
                    token_id: token_id.clone(),
                    msg: to_json_binary(&IbcOutgoingMsg {
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
            .unwrap();
        let event = res.events.into_iter().find(|e| e.ty == "wasm").unwrap();
        let class_data_attribute = event
            .attributes
            .into_iter()
            .find(|a| a.key == "class_data")
            .unwrap();
        let expected_contract_info: cosmwasm_std::ContractInfoResponse = from_json(
            &to_json_binary(&ContractInfoResponse {
                code_id: test.cw721_id,
                creator: test.minter.to_string(),
                admin: None,
                pinned: false,
                ibc_port: None,
            })
            .unwrap(),
        )
        .unwrap();
        let expected_collection_data = to_json_binary(&CollectionData {
            owner: Some(test.minter.to_string()),
            contract_info: expected_contract_info,
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            num_tokens: 1,
        })
        .unwrap();
        assert_eq!(
            class_data_attribute.value,
            format!("{:?}", expected_collection_data)
        );
    }
}

/// Tests that receiving a NFT via a regular receive fails when a
/// proxy is installed.
#[test]
fn test_no_receive_with_proxy() {
    let mut test = Test::new(true, None, cw721_base_contract());

    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("cw721"),
            test.ics721,
            &ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
                sender: test.app.api().addr_make("ekez").to_string(),
                token_id: "1".to_string(),
                msg: to_json_binary(&IbcOutgoingMsg {
                    receiver: test.app.api().addr_make("ekez").to_string(),
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
    let mut test = Test::new(true, Some("ekez".to_string()), cw721_base_contract());

    // Should start unpaused.
    let (paused, pauser) = test.query_pause_info();
    assert!(!paused);
    assert_eq!(pauser, Some(test.app.api().addr_make("ekez")));

    // Non-pauser may not pause.
    let err = test.pause_ics721_should_fail(test.app.api().addr_make("zeke").as_str());
    assert_eq!(
        err,
        ContractError::Pause(PauseError::Unauthorized {
            sender: test.app.api().addr_make("zeke")
        })
    );

    // Pause the ICS721 contract.
    test.pause_ics721(test.app.api().addr_make("ekez").as_str());
    // Pausing should remove the pauser.
    let (paused, pauser) = test.query_pause_info();
    assert!(paused);
    assert_eq!(pauser, None);

    // Pausing fails.
    let err = test.pause_ics721_should_fail(test.app.api().addr_make("ekez").as_str());
    assert_eq!(err, ContractError::Pause(PauseError::Paused {}));

    // Even something like executing a callback on ourselves will be
    // caught by a pause.
    let err: ContractError = test
        .app
        .execute_contract(
            test.ics721.clone(),
            test.ics721.clone(),
            &ExecuteMsg::Callback(CallbackMsg::Conjunction { operands: vec![] }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Pause(PauseError::Paused {}));

    // Set a new pauser.
    let ics721_id = test.app.store_code(ics721_contract());
    test.app
        .execute(
            test.app.api().addr_make("ekez"),
            WasmMsg::Migrate {
                contract_addr: test.ics721.to_string(),
                new_code_id: ics721_id,
                msg: to_json_binary(&MigrateMsg::WithUpdate {
                    pauser: Some(test.app.api().addr_make("zeke").to_string()),
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
    assert_eq!(pauser, Some(test.app.api().addr_make("zeke")));

    // One more pause for posterity sake.
    test.pause_ics721(test.app.api().addr_make("zeke").as_str());
    let (paused, pauser) = test.query_pause_info();
    assert!(paused);
    assert_eq!(pauser, None);
}

/// Tests migration.
#[test]
fn test_migration() {
    let mut test = Test::new(true, Some("ekez".to_string()), cw721_base_contract());
    // assert instantiation worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(pauser, Some(test.app.api().addr_make("ekez")));
    let proxy = test.query_proxy();
    assert!(proxy.is_some());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, test.cw721_id);

    // migrate changes
    test.app
        .execute(
            test.app.api().addr_make("ekez"),
            WasmMsg::Migrate {
                contract_addr: test.ics721.to_string(),
                new_code_id: test.ics721_id,
                msg: to_json_binary(&MigrateMsg::WithUpdate {
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
            test.app.api().addr_make("ekez"),
            WasmMsg::Migrate {
                contract_addr: test.ics721.to_string(),
                new_code_id: test.ics721_id,
                msg: to_json_binary(&MigrateMsg::WithUpdate {
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
