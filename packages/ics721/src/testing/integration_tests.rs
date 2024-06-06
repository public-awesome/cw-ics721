use anyhow::Result;
use bech32::{decode, encode, FromBase32, ToBase32, Variant};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json, instantiate2_address, to_json_binary, Addr, Api, Binary, CanonicalAddr, Deps,
    DepsMut, Empty, Env, GovMsg, IbcTimeout, IbcTimeoutBlock, MemoryStorage, MessageInfo,
    RecoverPubkeyError, Reply, Response, StdError, StdResult, Storage, VerificationError, WasmMsg,
};
use cw2::set_contract_version;
use cw721::{
    DefaultOptionalCollectionExtension, DefaultOptionalCollectionExtensionMsg,
    DefaultOptionalNftExtension, DefaultOptionalNftExtensionMsg,
};
use cw721_base::msg::{InstantiateMsg as Cw721InstantiateMsg, QueryMsg as Cw721QueryMsg};
use cw_cii::{Admin, ContractInstantiateInfo};
use cw_multi_test::{
    AddressGenerator, App, AppBuilder, BankKeeper, Contract, ContractWrapper, DistributionKeeper,
    Executor, FailingModule, IbcAcceptingModule, Router, StakeKeeper, StargateFailing, WasmKeeper,
};
use cw_pause_once::PauseError;
use sha2::{digest::Update, Digest, Sha256};

use crate::{
    execute::Ics721Execute,
    ibc::Ics721Ibc,
    msg::{CallbackMsg, ExecuteMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    query::Ics721Query,
    state::{CollectionData, UniversalAllNftInfoResponse},
    token_types::VoucherCreation,
    ContractError,
};
use ics721_types::{
    ibc_types::{IbcOutgoingMsg, IbcOutgoingProxyMsg},
    token_types::{Class, ClassId, Token, TokenId},
};

use super::contract::Ics721Contract;

const ICS721_CREATOR: &str = "ics721-creator";
const ICS721_ADMIN: &str = "ics721-admin";
const CONTRACT_NAME: &str = "crates.io:ics721-base";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

// owner, aka "minter"
const COLLECTION_OWNER_TARGET_CHAIN: &str = "collection-minter-target-chain";
const COLLECTION_OWNER_SOURCE_CHAIN: &str = "collection-minter-source-chain";
const COLLECTION_CONTRACT_SOURCE_CHAIN: &str = "collection-contract-source-chain";
const CHANNEL_TARGET_CHAIN: &str = "channel-1";
const BECH32_PREFIX_HRP: &str = "stars";
const NFT_OWNER_TARGET_CHAIN: &str = "nft-owner-target-chain";
const ICS721_ADMIN_AND_PAUSER: &str = "ics721-pauser";

type MockRouter = Router<
    BankKeeper,
    FailingModule<Empty, Empty, Empty>,
    WasmKeeper<Empty, Empty>,
    StakeKeeper,
    DistributionKeeper,
    IbcAcceptingModule,
    FailingModule<GovMsg, Empty, Empty>,
    StargateFailing,
>;

type MockApp = App<
    BankKeeper,
    MockApiBech32,
    MemoryStorage,
    FailingModule<Empty, Empty, Empty>,
    WasmKeeper<Empty, Empty>,
    StakeKeeper,
    DistributionKeeper,
    IbcAcceptingModule,
>;

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

fn query(deps: Deps, env: Env, msg: QueryMsg) -> Result<Binary, ContractError> {
    Ics721Contract::default().query(deps, env, msg)
}

fn migrate(deps: DepsMut, env: Env, msg: MigrateMsg) -> Result<Response, ContractError> {
    Ics721Contract::default().migrate(deps, env, msg)
}

fn no_init(_router: &mut MockRouter, _api: &dyn Api, _storage: &mut dyn Storage) {}

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
        let canonical_addr = instantiate2_address(checksum, creator, salt)?;
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
        Sha256::new()
            .chain(module)
            .chain(key)
            .finalize()
            .to_vec()
            .into()
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
        Err(StdError::generic_err(format!("Invalid input: {input}")))
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
    // even there is collection name, but it doesn't apply to CollectionData type
    pub name: String,
    // additional custom prop
    pub foo: Option<String>,
}

#[cw_serde]
pub struct PartialCustomCollectionData {
    pub owner: Option<String>,
    pub name: String,
    pub symbol: String,
    // additional custom prop
    pub foo: Option<String>,
    pub bar: String,
}

struct Test {
    app: MockApp,
    // origin cw721 contract on source chain for interchain transfers to other target chains
    source_cw721_owner: Addr,
    source_cw721_id: u64,
    source_cw721: Addr,
    // depending on test cast, it is ics721 either source or target chain
    ics721_id: u64,
    ics721: Addr,
    nfts_minted: usize,
}

impl Test {
    /// Test setup with optional pauser and proxy contracts.
    fn new(
        outgoing_proxy: bool,
        incoming_proxy: bool,
        channels: Option<Vec<String>>,
        admin_and_pauser: Option<String>,
        cw721_code: Box<dyn Contract<Empty>>,
        is_cw721_018: bool,
    ) -> Self {
        let mut app = AppBuilder::new()
            .with_wasm::<WasmKeeper<Empty, Empty>>(
                WasmKeeper::new().with_address_generator(MockAddressGenerator),
            )
            .with_ibc(IbcAcceptingModule::default())
            .with_api(MockApiBech32::new(BECH32_PREFIX_HRP))
            .build(no_init);
        let source_cw721_id = app.store_code(cw721_code);
        let ics721_id = app.store_code(ics721_contract());

        let outgoing_proxy = match outgoing_proxy {
            true => {
                let proxy_id = app.store_code(outgoing_proxy_contract());
                Some(ContractInstantiateInfo {
                    code_id: proxy_id,
                    msg: to_json_binary(
                        &cw_ics721_outgoing_proxy_rate_limit::msg::InstantiateMsg {
                            rate_limit: cw_ics721_outgoing_proxy_rate_limit::Rate::PerBlock(10),
                            origin: None,
                        },
                    )
                    .unwrap(),
                    admin: Some(Admin::Instantiator {}),
                    label: "outgoing proxy rate limit".to_string(),
                })
            }
            false => None,
        };

        let incoming_proxy = match incoming_proxy {
            true => {
                let proxy_id = app.store_code(incoming_proxy_contract());
                Some(ContractInstantiateInfo {
                    code_id: proxy_id,
                    msg: to_json_binary(&cw_ics721_incoming_proxy_base::msg::InstantiateMsg {
                        origin: None,
                        channels,
                    })
                    .unwrap(),
                    admin: Some(Admin::Instantiator {}),
                    label: "incoming proxy".to_string(),
                })
            }
            false => None,
        };

        let admin = admin_and_pauser
            .clone()
            .map(|p| app.api().addr_make(&p).to_string());
        let ics721 = app
            .instantiate_contract(
                ics721_id,
                app.api().addr_make(ICS721_CREATOR),
                &InstantiateMsg {
                    cw721_base_code_id: source_cw721_id,
                    incoming_proxy,
                    outgoing_proxy,
                    pauser: admin.clone(),
                    cw721_admin: admin.clone(),
                    contract_addr_length: None,
                },
                &[],
                "ics721-base",
                admin.clone(),
            )
            .unwrap();

        let source_cw721_owner = app.api().addr_make(COLLECTION_OWNER_SOURCE_CHAIN);
        let source_cw721 = match is_cw721_018 {
            true => app
                .instantiate_contract(
                    source_cw721_id,
                    source_cw721_owner.clone(),
                    &Cw721InstantiateMsg::<DefaultOptionalCollectionExtensionMsg> {
                        name: "name".to_string(),
                        symbol: "symbol".to_string(),
                        minter: Some(source_cw721_owner.to_string()),
                        creator: None,
                        collection_info_extension: None,
                        withdraw_address: None,
                    },
                    &[],
                    "cw721-base",
                    None,
                )
                .unwrap(),
            false => app
                .instantiate_contract(
                    source_cw721_id,
                    source_cw721_owner.clone(),
                    &cw721_base_016::msg::InstantiateMsg {
                        name: "name".to_string(),
                        symbol: "symbol".to_string(),
                        minter: source_cw721_owner.to_string(),
                    },
                    &[],
                    "cw721-base",
                    None,
                )
                .unwrap(),
        };

        Self {
            app,
            source_cw721_owner,
            source_cw721_id,
            source_cw721,
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

    fn query_outgoing_proxy(&mut self) -> Option<Addr> {
        self.app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::OutgoingProxy {})
            .unwrap()
    }

    fn query_incoming_proxy(&mut self) -> Option<Addr> {
        self.app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::IncomingProxy {})
            .unwrap()
    }

    fn query_cw721_id(&mut self) -> u64 {
        self.app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::Cw721CodeId {})
            .unwrap()
    }

    fn query_cw721_admin(&mut self) -> Option<Addr> {
        self.app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::Cw721Admin {})
            .unwrap()
    }

    fn query_contract_addr_length(&mut self) -> Option<u32> {
        self.app
            .wrap()
            .query_wasm_smart(self.ics721.clone(), &QueryMsg::ContractAddrLength {})
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

    fn query_cw721_all_nft_info(&mut self, token_id: String) -> UniversalAllNftInfoResponse {
        self.app
            .wrap()
            .query_wasm_smart(
                self.source_cw721.clone(),
                &cw721_base::msg::QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::AllNftInfo {
                    token_id,
                    include_expired: None,
                },
            )
            .unwrap()
    }

    fn execute_cw721_mint(&mut self, owner: Addr) -> Result<String, anyhow::Error> {
        self.nfts_minted += 1;

        self.app
            .execute_contract(
                self.source_cw721_owner.clone(),
                self.source_cw721.clone(),
                &cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::Mint {
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

fn incoming_proxy_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_ics721_incoming_proxy_base::contract::execute,
        cw_ics721_incoming_proxy_base::contract::instantiate,
        cw_ics721_incoming_proxy_base::contract::query,
    );
    Box::new(contract)
}

fn outgoing_proxy_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw_ics721_outgoing_proxy_rate_limit::contract::execute,
        cw_ics721_outgoing_proxy_rate_limit::contract::instantiate,
        cw_ics721_outgoing_proxy_rate_limit::contract::query,
    );
    Box::new(contract)
}

#[test]
fn test_instantiate() {
    let mut test = Test::new(
        true,
        true,
        None,
        Some(ICS721_ADMIN.to_string()),
        cw721_base_contract(),
        true,
    );

    // check stores are properly initialized
    let cw721_id = test.query_cw721_id();
    assert_eq!(cw721_id, test.source_cw721_id);
    let nft_contracts: Vec<(String, Addr)> = test.query_nft_contracts();
    assert_eq!(nft_contracts, Vec::<(String, Addr)>::new());
    let outgoing_channels = test.query_outgoing_channels();
    assert_eq!(outgoing_channels, []);
    let incoming_channels = test.query_incoming_channels();
    assert_eq!(incoming_channels, []);
    let outgoing_proxy = test.query_outgoing_proxy();
    assert!(outgoing_proxy.is_some());
    let incoming_proxy = test.query_incoming_proxy();
    assert!(incoming_proxy.is_some());
    let cw721_admin = test.query_cw721_admin();
    assert_eq!(cw721_admin, Some(test.app.api().addr_make(ICS721_ADMIN)));
}

#[test]
fn test_do_instantiate_and_mint_weird_data() {
    let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
    let collection_contract_source_chain =
        ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
    let class_id = format!(
        "wasm.{}/{}/{}",
        test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain
    );
    test.app
        .execute_contract(
            test.ics721.clone(),
            test.ics721,
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new(class_id),
                        uri: None,
                        data: Some(
                            to_json_binary(&CollectionData {
                                owner: Some(
                                    // incoming collection data from source chain
                                    test.app
                                        .api()
                                        .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                                        .to_string(),
                                ),
                                contract_info: Default::default(),
                                name: "name".to_string(),
                                symbol: "symbol".to_string(),
                                num_tokens: Some(1),
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
    // test case: instantiate cw721 with no ClassData (without owner, name, and symbol)
    {
        let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
        let collection_contract_source_chain =
            ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
        let class_id = format!(
            "wasm.{}/{}/{}",
            test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain
        );
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new(class_id.clone()),
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
                                uri: Some("https://foo.bar".to_string()),
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
        assert_eq!(nft_contracts[0].0, class_id);
        // Get the address of the instantiated NFT.
        let nft_contract: Addr = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();

        // check name and symbol contains class id for instantiated nft contract
        #[allow(deprecated)]
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::msg::CollectionInfoAndExtensionResponse::<DefaultOptionalCollectionExtension> {
                name: class_id.to_string(),   // name is set to class_id
                symbol: class_id.to_string(), // symbol is set to class_id
                extension: None,
                updated_at: contract_info.updated_at, // ignore this field
            }
        );

        // Check that token_uri was set properly.
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            token_info.token_uri,
            Some("https://moonphase.is/image.svg".to_string())
        );
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "2".to_string(),
                },
            )
            .unwrap();
        assert_eq!(token_info.token_uri, Some("https://foo.bar".to_string()));

        // After transfer to target, test owner can do any action, like transfer, on collection
        test.app
            .execute_contract(
                test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN),
                nft_contract.clone(),
                &cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::TransferNft {
                    recipient: nft_contract.to_string(),
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id,
                },
            )
            .unwrap();
        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        assert_eq!(base_owner, owner);
    }
    // test case: instantiate cw721 with ClassData containing owner, name, and symbol
    {
        let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
        let collection_contract_source_chain =
            ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
        let class_id = format!(
            "wasm.{}/{}/{}",
            test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain
        );
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new(class_id.clone()),
                            uri: Some("https://moonphase.is".to_string()),
                            data: Some(
                                to_json_binary(&CollectionData {
                                    owner: Some(
                                        // incoming collection data from source chain
                                        test.app
                                            .api()
                                            .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                                            .to_string(),
                                    ),
                                    contract_info: Default::default(),
                                    name: "ark".to_string(),
                                    symbol: "protocol".to_string(),
                                    num_tokens: Some(1),
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
                                uri: Some("https://foo.bar".to_string()),
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
        assert_eq!(nft_contracts[0].0, class_id);
        // Get the address of the instantiated NFT.
        let nft_contract: Addr = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();

        // check name and symbol is using class data for instantiated nft contract
        #[allow(deprecated)]
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::msg::CollectionInfoAndExtensionResponse::<DefaultOptionalCollectionExtension> {
                name: "ark".to_string(),
                symbol: "protocol".to_string(),
                extension: None,
                updated_at: contract_info.updated_at, // ignore this field
            }
        );

        // Check that token_uri was set properly.
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            token_info.token_uri,
            Some("https://moonphase.is/image.svg".to_string())
        );
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "2".to_string(),
                },
            )
            .unwrap();
        assert_eq!(token_info.token_uri, Some("https://foo.bar".to_string()));

        // After transfer to target, test owner can do any action, like transfer, on collection
        test.app
            .execute_contract(
                test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN),
                nft_contract.clone(), // new recipient
                &cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::TransferNft {
                    recipient: nft_contract.to_string(),
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id,
                },
            )
            .unwrap();
        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        assert_eq!(base_owner, owner);
    }
    // test case: instantiate cw721 with CustomClassData (includes name, but without owner and symbol)
    // results in nft contract using class id for name and symbol
    {
        let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
        let collection_contract_source_chain =
            ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
        let class_id = format!(
            "wasm.{}/{}/{}",
            test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain
        );
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new(class_id.clone()),
                            uri: Some("https://moonphase.is".to_string()),
                            data: Some(
                                // CustomClassData doesn't apply to CollectionData type and won't be considered
                                // collection name wont be transferred to instantiated nft contract
                                to_json_binary(&CustomClassData {
                                    foo: Some(
                                        test.app
                                            .api()
                                            .addr_make(COLLECTION_OWNER_TARGET_CHAIN)
                                            .to_string(),
                                    ),
                                    name: "colection-name".to_string(),
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
                                uri: Some("https://foo.bar".to_string()),
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
        assert_eq!(nft_contracts[0].0, class_id);
        // Get the address of the instantiated NFT.
        let nft_contract: Addr = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();

        // check name and symbol contains class id for instantiated nft contract
        let contract_info: cw721::msg::CollectionInfoAndExtensionResponse<
            DefaultOptionalCollectionExtension,
        > = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::GetCollectionInfoAndExtension {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::msg::CollectionInfoAndExtensionResponse::<DefaultOptionalCollectionExtension> {
                name: class_id.to_string(),
                symbol: class_id.to_string(),
                extension: None,
                updated_at: contract_info.updated_at, // ignore this field
            }
        );

        // Check that token_uri was set properly.
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            token_info.token_uri,
            Some("https://moonphase.is/image.svg".to_string())
        );
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "2".to_string(),
                },
            )
            .unwrap();
        assert_eq!(token_info.token_uri, Some("https://foo.bar".to_string()));

        // After transfer to target, test owner can do any action, like transfer, on collection
        test.app
            .execute_contract(
                test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN),
                nft_contract.clone(),
                &cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::TransferNft {
                    recipient: nft_contract.to_string(), // new owner
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id,
                },
            )
            .unwrap();

        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        assert_eq!(base_owner, owner);
    }
    // test case: instantiate cw721 with PartialCustomCollectionData (includes name and symbol)
    // results in nft contract using name and symbol
    {
        let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
        let collection_contract_source_chain =
            ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
        let class_id = format!(
            "wasm.{}/{}/{}",
            test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain
        );
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new(class_id.clone()),
                            uri: Some("https://moonphase.is".to_string()),
                            data: Some(
                                // CustomClassData doesn't apply to CollectionData type and won't be considered
                                // collection name wont be transferred to instantiated nft contract
                                to_json_binary(&PartialCustomCollectionData {
                                    owner: None,
                                    name: "collection-name".to_string(),
                                    symbol: "collection-symbol".to_string(),
                                    bar: "bar".to_string(),
                                    foo: Some(
                                        test.app
                                            .api()
                                            .addr_make(COLLECTION_OWNER_TARGET_CHAIN)
                                            .to_string(),
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
                                uri: Some("https://foo.bar".to_string()),
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
        assert_eq!(nft_contracts[0].0, class_id);
        // Get the address of the instantiated NFT.
        let nft_contract: Addr = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();

        // check name and symbol contains class id for instantiated nft contract
        let contract_info: cw721::msg::CollectionInfoAndExtensionResponse<
            DefaultOptionalCollectionExtension,
        > = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                #[allow(deprecated)]
                &Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::msg::CollectionInfoAndExtensionResponse::<DefaultOptionalCollectionExtension> {
                name: "collection-name".to_string(),
                symbol: "collection-symbol".to_string(),
                extension: None,
                updated_at: contract_info.updated_at, // ignore this field
            }
        );

        // Check that token_uri was set properly.
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            token_info.token_uri,
            Some("https://moonphase.is/image.svg".to_string())
        );
        let token_info: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "2".to_string(),
                },
            )
            .unwrap();
        assert_eq!(token_info.token_uri, Some("https://foo.bar".to_string()));

        // After transfer to target, test owner can do any action, like transfer, on collection
        test.app
            .execute_contract(
                test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN),
                nft_contract.clone(),
                &cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::TransferNft {
                    recipient: nft_contract.to_string(), // new owner
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id,
                },
            )
            .unwrap();

        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        assert_eq!(base_owner, owner);
    }
}

#[test]
fn test_do_instantiate_and_mint_2_different_collections() {
    // test case: instantiate two cw721 contracts with different class id and make sure instantiate2 creates 2 different, predictable contracts
    {
        let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
        let collection_contract_source_chain_1 =
            ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
        let class_id_1 = format!(
            "wasm.{}/{}/{}",
            test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain_1
        );
        let collection_contract_source_chain_2 = ClassId::new(test.app.api().addr_make("other"));
        let class_id_2 = format!(
            "wasm.{}/{}/{}",
            test.ics721, "channel-123567890", collection_contract_source_chain_2
        );

        // create contract 1
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new(class_id_1.clone()),
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
                                uri: Some("https://foo.bar".to_string()),
                                data: None,
                            },
                        ],
                    },
                }),
                &[],
            )
            .unwrap();
        // create contract 2
        test.app
            .execute_contract(
                test.ics721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                    receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                    create: VoucherCreation {
                        class: Class {
                            id: ClassId::new(class_id_2.clone()),
                            uri: Some("https://moonphase.is".to_string()),
                            data: None, // no class data
                        },
                        tokens: vec![
                            Token {
                                id: TokenId::new("1"),
                                uri: Some("https://mr.t".to_string()),
                                data: None,
                            },
                            Token {
                                id: TokenId::new("2"),
                                uri: Some("https://ark.protocol".to_string()),
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
        assert_eq!(nft_contracts.len(), 2);
        assert_eq!(nft_contracts[0].0, class_id_1);
        assert_eq!(nft_contracts[1].0, class_id_2);
        // Get the address of the instantiated NFT.
        let nft_contract_1: Addr = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: class_id_1.to_string(),
                },
            )
            .unwrap();
        let nft_contract_2: Addr = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::NftContract {
                    class_id: class_id_2.to_string(),
                },
            )
            .unwrap();

        // check name and symbol contains class id for instantiated nft contract
        let contract_info_1: cw721::msg::CollectionInfoAndExtensionResponse<
            DefaultOptionalCollectionExtension,
        > = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1.clone(),
                #[allow(deprecated)]
                &Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::ContractInfo {},
            )
            .unwrap();
        let contract_info_2: cw721::msg::CollectionInfoAndExtensionResponse<
            DefaultOptionalCollectionExtension,
        > = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2.clone(),
                #[allow(deprecated)]
                &Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info_1,
            cw721::msg::CollectionInfoAndExtensionResponse::<DefaultOptionalCollectionExtension> {
                name: class_id_1.to_string(),   // name is set to class_id
                symbol: class_id_1.to_string(), // symbol is set to class_id
                extension: None,
                updated_at: contract_info_1.updated_at, // ignore this field
            }
        );
        assert_eq!(
            contract_info_2,
            cw721::msg::CollectionInfoAndExtensionResponse::<DefaultOptionalCollectionExtension> {
                name: class_id_2.to_string(),   // name is set to class_id
                symbol: class_id_2.to_string(), // symbol is set to class_id
                extension: None,
                updated_at: contract_info_2.updated_at, // ignore this field
            }
        );

        // Check that token_uri was set properly.
        let token_info_1_1: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        let token_info_2_1: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            token_info_1_1.token_uri,
            Some("https://moonphase.is/image.svg".to_string())
        );
        assert_eq!(token_info_2_1.token_uri, Some("https://mr.t".to_string()));
        let token_info_1_2: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "2".to_string(),
                },
            )
            .unwrap();
        let token_info_2_2: cw721::msg::NftInfoResponse<DefaultOptionalNftExtension> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2.clone(),
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::NftInfo {
                    token_id: "2".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            token_info_1_2.token_uri,
            Some("https://foo.bar".to_string())
        );
        assert_eq!(
            token_info_2_2.token_uri,
            Some("https://ark.protocol".to_string())
        );

        // After transfer to target, test owner can do any action, like transfer, on collection
        test.app
            .execute_contract(
                test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN),
                nft_contract_1.clone(),
                &cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::TransferNft {
                    recipient: nft_contract_1.to_string(),
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();
        test.app
            .execute_contract(
                test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN),
                nft_contract_2.clone(),
                &cw721_base::msg::ExecuteMsg::<
                    DefaultOptionalNftExtensionMsg,
                    DefaultOptionalCollectionExtensionMsg,
                    Empty,
                >::TransferNft {
                    recipient: nft_contract_2.to_string(),
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner_1: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id_1,
                },
            )
            .unwrap();
        let owner_2: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id_2,
                },
            )
            .unwrap();
        assert_eq!(owner_1.owner, nft_contract_1.to_string());
        assert_eq!(owner_2.owner, nft_contract_2.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner_1: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        let base_owner_2: cw721::msg::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2,
                &cw721::msg::Cw721QueryMsg::<
                    DefaultOptionalNftExtension,
                    DefaultOptionalCollectionExtension,
                    Empty,
                >::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        assert_eq!(base_owner_1, owner_1);
        assert_eq!(base_owner_2, owner_2);
    }
}

#[test]
fn test_do_instantiate_and_mint_no_instantiate() {
    let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
    let collection_contract_source_chain =
        ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
    let class_id = format!(
        "wasm.{}/{}/{}",
        test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain
    );
    // Check calling CreateVouchers twice with same class id
    // on 2nd call it will not instantiate a new contract,
    // instead it will just mint the token on existing contract
    test.app
        .execute_contract(
            test.ics721.clone(),
            test.ics721.clone(),
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new(class_id.clone()),
                        uri: Some("https://moonphase.is".to_string()),
                        data: Some(
                            to_json_binary(&CollectionData {
                                owner: Some(
                                    // incoming collection data from source chain
                                    test.app
                                        .api()
                                        .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                                        .to_string(),
                                ),
                                contract_info: Default::default(),
                                name: "name".to_string(),
                                symbol: "symbol".to_string(),
                                num_tokens: Some(1),
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
    assert_eq!(class_id_to_nft_contract[0].0, class_id);

    // 2nd call will only do a mint as the contract for the class ID has
    // already been instantiated.
    test.app
        .execute_contract(
            test.ics721.clone(),
            test.ics721.clone(),
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new(class_id.clone()),
                        uri: Some("https://moonphase.is".to_string()),
                        // unlike above in 1st transfer, here on 2nd transfer no classdata is provided!
                        // this won't affect collection since it's already instantiated
                        data: None,
                    },
                    tokens: vec![Token {
                        id: TokenId::new("2"),
                        uri: Some("https://foo.bar".to_string()),
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
    let nft_contract: Addr = test
        .app
        .wrap()
        .query_wasm_smart(test.ics721, &QueryMsg::NftContract { class_id })
        .unwrap();

    // Make sure we have our tokens.
    let tokens: cw721::msg::TokensResponse = test
        .app
        .wrap()
        .query_wasm_smart(
            nft_contract,
            &cw721::msg::Cw721QueryMsg::<
                DefaultOptionalNftExtension,
                DefaultOptionalCollectionExtension,
                Empty,
            >::AllTokens {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();
    assert_eq!(tokens.tokens, vec!["1".to_string(), "2".to_string()])
}

#[test]
fn test_do_instantiate_and_mint_permissions() {
    let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
    let collection_contract_source_chain =
        ClassId::new(test.app.api().addr_make(COLLECTION_CONTRACT_SOURCE_CHAIN));
    let class_id = format!(
        "wasm.{}/{}/{}",
        test.ics721, CHANNEL_TARGET_CHAIN, collection_contract_source_chain
    );
    // Method is only callable by the contract itself.
    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("notIcs721"),
            test.ics721,
            &ExecuteMsg::Callback(CallbackMsg::CreateVouchers {
                receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                create: VoucherCreation {
                    class: Class {
                        id: ClassId::new(class_id),
                        uri: Some("https://moonphase.is".to_string()),
                        data: Some(
                            to_json_binary(&CollectionData {
                                owner: Some(
                                    // incoming collection data from source chain
                                    test.app
                                        .api()
                                        .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                                        .to_string(),
                                ),
                                contract_info: Default::default(),
                                name: "name".to_string(),
                                symbol: "symbol".to_string(),
                                num_tokens: Some(1),
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

/// Tests that we can not send IbcOutgoingProxyMsg if no proxy is configured.
#[test]
fn test_no_proxy_unknown_msg() {
    let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
    let msg = IbcOutgoingProxyMsg {
        collection: "foo".to_string(),
        msg: to_json_binary(&IbcOutgoingMsg {
            receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
            channel_id: "channel-0".to_string(),
            timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                revision: 0,
                height: 10,
            }),
            memo: None,
        })
        .unwrap(),
    };
    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("proxy"),
            test.ics721,
            &ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                sender: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                token_id: "1".to_string(),
                msg: to_json_binary(&msg).unwrap(),
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(
        err,
        ContractError::UnknownMsg(to_json_binary(&msg).unwrap())
    );
}

/// Tests that we can non-proxy addresses can send if proxy is configured.
#[test]
fn test_no_proxy_unauthorized() {
    let mut test = Test::new(true, false, None, None, cw721_base_contract(), true);
    let msg = IbcOutgoingProxyMsg {
        collection: "foo".to_string(),
        msg: to_json_binary(&IbcOutgoingMsg {
            receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
            channel_id: "channel-0".to_string(),
            timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                revision: 0,
                height: 10,
            }),
            memo: None,
        })
        .unwrap(),
    };
    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("foo"),
            test.ics721,
            &ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                sender: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                token_id: "1".to_string(),
                msg: to_json_binary(&msg).unwrap(),
            }),
            &[],
        )
        .unwrap_err()
        .downcast()
        .unwrap();
    assert_eq!(err, ContractError::Unauthorized {});
}

#[test]
fn test_proxy_authorized() {
    let mut test = Test::new(true, false, None, None, cw721_base_contract(), true);
    let proxy_address: Option<Addr> = test
        .app
        .wrap()
        .query_wasm_smart(&test.ics721, &QueryMsg::OutgoingProxy {})
        .unwrap();
    // check proxy is set
    let proxy_address = proxy_address.expect("expected a proxy");

    // create collection and mint NFT for sending to proxy
    let source_cw721_id = test.app.store_code(cw721_base_contract());
    let source_cw721 = test
        .app
        .instantiate_contract(
            source_cw721_id,
            test.app.api().addr_make("ekez"),
            &cw721_base::msg::InstantiateMsg::<DefaultOptionalCollectionExtensionMsg> {
                name: "token".to_string(),
                symbol: "nonfungible".to_string(),
                minter: Some(
                    test.app
                        .api()
                        .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                        .to_string(),
                ),
                creator: None,
                collection_info_extension: None,
                withdraw_address: None,
            },
            &[],
            "label cw721",
            None,
        )
        .unwrap();
    // simplify: instead of `send_nft` to proxy, and proxy transfer NFT to ics721 and call receive proxy,
    // here it is directly transferred to ics721 and then call receive proxy
    test.app
        .execute_contract(
            test.app.api().addr_make(COLLECTION_OWNER_SOURCE_CHAIN),
            source_cw721.clone(),
            &cw721_base::msg::ExecuteMsg::<
                DefaultOptionalNftExtensionMsg,
                DefaultOptionalCollectionExtensionMsg,
                Empty,
            >::Mint {
                token_id: "1".to_string(),
                owner: test.ics721.to_string(),
                token_uri: None,
                extension: None,
            },
            &[],
        )
        .unwrap();

    // ics721 receives NFT from proxy,
    // if - and only if - nft is escrowed by ics721, ics721 will interchain transfer NFT!
    // otherwise proxy is unauthorised to transfer NFT
    test.app
        .execute_contract(
            proxy_address,
            test.ics721,
            &ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                sender: test
                    .app
                    .api()
                    .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                    .to_string(),
                token_id: "1".to_string(),
                msg: to_json_binary(&IbcOutgoingProxyMsg {
                    collection: source_cw721.into_string(),
                    msg: to_json_binary(&IbcOutgoingMsg {
                        receiver: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                        channel_id: "channel-0".to_string(),
                        timeout: IbcTimeout::with_block(IbcTimeoutBlock {
                            revision: 0,
                            height: 10,
                        }),
                        memo: None,
                    })
                    .unwrap(),
                })
                .unwrap(),
            }),
            &[],
        )
        .unwrap();
}

#[test]
fn test_receive_nft() {
    // test case: receive nft from cw721-base
    {
        let mut test = Test::new(false, false, None, None, cw721_base_contract(), true);
        // simplify: mint and escrowed/owned by ics721, as a precondition for receive nft
        let token_id = test.execute_cw721_mint(test.ics721.clone()).unwrap();
        // ics721 receives NFT from sender/collection contract,
        // if - and only if - nft is escrowed by ics721, ics721 will interchain transfer NFT!
        // otherwise proxy is unauthorised to transfer NFT
        let res = test
            .app
            .execute_contract(
                test.source_cw721.clone(),
                test.ics721,
                &ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                    sender: test.source_cw721_owner.to_string(),
                    token_id,
                    msg: to_json_binary(&IbcOutgoingMsg {
                        receiver: NFT_OWNER_TARGET_CHAIN.to_string(), // nft owner for other chain, on this chain ics721 is owner
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

        // get class data (containing collection data) from response
        let event = res.events.into_iter().find(|e| e.ty == "wasm").unwrap();
        let class_data_attribute = event
            .attributes
            .into_iter()
            .find(|a| a.key == "class_data")
            .unwrap();
        // check collection data matches with data from source nft contract
        let expected_contract_info: cosmwasm_std::ContractInfoResponse =
        // workaround using from_json/to_json_binary since ContractInfoResponse is non-exhaustive, can't be created directly
        from_json(
            to_json_binary(&ContractInfoResponse {
                code_id: test.source_cw721_id,
                creator: test.source_cw721_owner.to_string(),
                admin: None,
                pinned: false,
                ibc_port: None,
            })
            .unwrap(),
        )
        .unwrap();
        let expected_collection_data = to_json_binary(&CollectionData {
            owner: Some(
                // collection data from source chain
                test.source_cw721_owner.to_string(),
            ),
            contract_info: Some(expected_contract_info),
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            num_tokens: Some(1),
        })
        .unwrap();
        assert_eq!(
            class_data_attribute.value,
            format!("{expected_collection_data:?}")
        );
    }
    // test case: backward compatibility - receive nft also works for old/v016 cw721-base
    {
        let mut test = Test::new(false, false, None, None, cw721_v016_base_contract(), false);
        // simplify: mint and escrowed/owned by ics721, as a precondition for receive nft
        let token_id = test.execute_cw721_mint(test.ics721.clone()).unwrap();
        // ics721 receives NFT from sender/collection contract,
        // if - and only if - nft is escrowed by ics721, ics721 will interchain transfer NFT!
        // otherwise proxy is unauthorised to transfer NFT
        let res = test
            .app
            .execute_contract(
                test.source_cw721.clone(),
                test.ics721,
                &ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                    sender: test.source_cw721_owner.to_string(),
                    token_id,
                    msg: to_json_binary(&IbcOutgoingMsg {
                        receiver: NFT_OWNER_TARGET_CHAIN.to_string(), // nft owner for other chain, on this chain ics721 is owner
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

        // get class data (containing collection data) from response
        let event = res.events.into_iter().find(|e| e.ty == "wasm").unwrap();
        let class_data_attribute = event
            .attributes
            .into_iter()
            .find(|a| a.key == "class_data")
            .unwrap();
        // check collection data matches with data from source nft contract
        let expected_contract_info: cosmwasm_std::ContractInfoResponse =
        // workaround using from_json/to_json_binary since ContractInfoResponse is non-exhaustive, can't be created directly
        from_json(
            to_json_binary(&ContractInfoResponse {
                code_id: test.source_cw721_id,
                creator: test.source_cw721_owner.to_string(),
                admin: None,
                pinned: false,
                ibc_port: None,
            })
            .unwrap(),
        )
        .unwrap();
        let expected_collection_data = to_json_binary(&CollectionData {
            owner: Some(
                // collection data from source chain
                test.app
                    .api()
                    .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                    .to_string(),
            ),
            contract_info: Some(expected_contract_info),
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            num_tokens: Some(1),
        })
        .unwrap();
        assert_eq!(
            class_data_attribute.value,
            format!("{expected_collection_data:?}")
        );
    }
}

#[test]
fn test_admin_clean_and_unescrow_nft() {
    // test case: receive nft from cw721-base
    {
        let mut test = Test::new(
            false,
            false,
            None,
            Some(ICS721_ADMIN_AND_PAUSER.to_string()),
            cw721_base_contract(),
            true,
        );
        // simplify: mint and escrowed/owned by ics721, as a precondition for receive nft
        let token_id_escrowed_by_ics721 = test.execute_cw721_mint(test.ics721.clone()).unwrap();
        let recipient = test.app.api().addr_make("recipient");
        let token_id_from_owner = test.execute_cw721_mint(recipient.clone()).unwrap();
        let channel = "channel-0".to_string();
        test.app
            .execute_contract(
                test.source_cw721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                    sender: test.source_cw721_owner.to_string(),
                    token_id: token_id_escrowed_by_ics721.clone(),
                    msg: to_json_binary(&IbcOutgoingMsg {
                        receiver: NFT_OWNER_TARGET_CHAIN.to_string(), // nft owner for other chain, on this chain ics721 is owner
                        channel_id: channel.clone(),
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
        // check outgoing channel entry
        let outgoing_channel = test.query_outgoing_channels();
        assert_eq!(outgoing_channel.len(), 1);
        let class_id = ClassId::new(test.source_cw721.to_string());
        assert_eq!(
            outgoing_channel,
            vec![(
                (class_id.to_string(), token_id_escrowed_by_ics721.clone()),
                channel.clone()
            )]
        );
        // assert nft is escrowed
        let UniversalAllNftInfoResponse { access, .. } =
            test.query_cw721_all_nft_info(token_id_escrowed_by_ics721.clone());
        assert_eq!(access.owner, test.ics721.to_string());

        // non admin can't call
        let non_admin = test.app.api().addr_make("not_admin");
        let admin = test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER);
        let clean_and_burn_msg = ExecuteMsg::AdminCleanAndBurnNft {
            owner: recipient.to_string(),
            token_id: token_id_escrowed_by_ics721.clone(),
            class_id: class_id.to_string(),
            collection: test.source_cw721.to_string(),
        };
        let err: ContractError = test
            .app
            .execute_contract(
                non_admin.clone(),
                test.ics721.clone(),
                &clean_and_burn_msg,
                &[],
            )
            .unwrap_err()
            .downcast()
            .unwrap();
        assert_eq!(err, ContractError::Unauthorized {});

        let clean_and_unescrow_msg = ExecuteMsg::AdminCleanAndUnescrowNft {
            recipient: recipient.to_string(),
            token_id: token_id_from_owner.clone(), // not escrowed by ics721
            class_id: class_id.to_string(),
            collection: test.source_cw721.to_string(),
        };
        let err: ContractError = test
            .app
            .execute_contract(
                admin.clone(),
                test.ics721.clone(),
                &clean_and_unescrow_msg,
                &[],
            )
            .unwrap_err()
            .downcast()
            .unwrap();
        assert_eq!(
            err,
            ContractError::NotEscrowedByIcs721(recipient.to_string())
        );

        // unknown class id
        let clean_and_unescrow_msg = ExecuteMsg::AdminCleanAndUnescrowNft {
            recipient: recipient.to_string(),
            token_id: token_id_escrowed_by_ics721.to_string(),
            class_id: "unknown".to_string(),
            collection: test.source_cw721.to_string(),
        };
        let err: ContractError = test
            .app
            .execute_contract(
                admin.clone(),
                test.ics721.clone(),
                &clean_and_unescrow_msg,
                &[],
            )
            .unwrap_err()
            .downcast()
            .unwrap();
        assert_eq!(
            err,
            ContractError::NoNftContractForClassId("unknown".to_string())
        );

        let clean_and_unescrow_msg = ExecuteMsg::AdminCleanAndUnescrowNft {
            recipient: recipient.to_string(),
            token_id: token_id_escrowed_by_ics721.clone(),
            class_id: class_id.to_string(),
            collection: test.source_cw721.to_string(),
        };
        test.app
            .execute_contract(
                admin.clone(),
                test.ics721.clone(),
                &clean_and_unescrow_msg,
                &[],
            )
            .unwrap();
        // asert outgoing channel entry is removed
        let outgoing_channel = test.query_outgoing_channels();
        assert_eq!(outgoing_channel.len(), 0);
        // check nft is unescrowed
        let UniversalAllNftInfoResponse { access, .. } =
            test.query_cw721_all_nft_info(token_id_escrowed_by_ics721.clone());
        assert_eq!(access.owner, recipient.to_string());
    }
}

/// In case proxy for ICS721 is defined, ICS721 only accepts receival from proxy - not from nft contract!
#[test]
fn test_no_receive_with_proxy() {
    let mut test = Test::new(true, false, None, None, cw721_base_contract(), true);
    // unauthorized to receive nft from nft contract
    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("cw721"),
            test.ics721,
            &ExecuteMsg::ReceiveNft(cw721::receiver::Cw721ReceiveMsg {
                sender: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
                token_id: "1".to_string(),
                msg: to_json_binary(&IbcOutgoingMsg {
                    receiver: NFT_OWNER_TARGET_CHAIN.to_string(), // nft owner for other chain, on this chain ics721 is owner
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
    let mut test = Test::new(
        true,
        false,
        None,
        Some(ICS721_ADMIN_AND_PAUSER.to_string()),
        cw721_base_contract(),
        true,
    );
    // Should start unpaused.
    let (paused, pauser) = test.query_pause_info();
    assert!(!paused);
    assert_eq!(
        pauser,
        Some(test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER))
    );

    // Non-pauser may not pause.
    let err = test.pause_ics721_should_fail(test.app.api().addr_make("zeke").as_str());
    assert_eq!(
        err,
        ContractError::Pause(PauseError::Unauthorized {
            sender: test.app.api().addr_make("zeke")
        })
    );

    // Pause the ICS721 contract.
    test.pause_ics721(test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER).as_str());
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

    // Pauser can pause only once, for another pause, a new pauser needs to be set via migration
    let ics721_id = test.app.store_code(ics721_contract());
    test.app
        .execute(
            test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER),
            WasmMsg::Migrate {
                contract_addr: test.ics721.to_string(),
                new_code_id: ics721_id,
                msg: to_json_binary(&MigrateMsg::WithUpdate {
                    pauser: Some(test.app.api().addr_make("new_pauser").to_string()),
                    incoming_proxy: None,
                    outgoing_proxy: None,
                    cw721_base_code_id: None,
                    cw721_admin: None,
                    contract_addr_length: None,
                })
                .unwrap(),
            }
            .into(),
        )
        .unwrap();

    // Setting new pauser should unpause.
    let (paused, pauser) = test.query_pause_info();
    assert!(!paused);
    assert_eq!(pauser, Some(test.app.api().addr_make("new_pauser")));

    // One more pause for posterity sake.
    test.pause_ics721(test.app.api().addr_make("new_pauser").as_str());
    let (paused, pauser) = test.query_pause_info();
    assert!(paused);
    assert_eq!(pauser, None);
}

/// Tests migration.
#[test]
fn test_migration() {
    let mut test = Test::new(
        true,
        false,
        None,
        Some(ICS721_ADMIN_AND_PAUSER.to_string()),
        cw721_base_contract(),
        true,
    );
    // assert instantiation worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(
        pauser,
        Some(test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER))
    );
    let outgoing_proxy = test.query_outgoing_proxy();
    assert!(outgoing_proxy.is_some());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, test.source_cw721_id);

    // migrate changes
    let admin = test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER);
    test.app
        .execute(
            admin.clone(),
            WasmMsg::Migrate {
                contract_addr: test.ics721.to_string(),
                new_code_id: test.ics721_id,
                msg: to_json_binary(&MigrateMsg::WithUpdate {
                    pauser: None,
                    incoming_proxy: None,
                    outgoing_proxy: None,
                    cw721_base_code_id: Some(12345678),
                    cw721_admin: Some(admin.to_string()),
                    contract_addr_length: Some(20),
                })
                .unwrap(),
            }
            .into(),
        )
        .unwrap();
    // assert migration worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(pauser, None);
    let proxy = test.query_outgoing_proxy();
    assert!(proxy.is_none());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, 12345678);
    assert_eq!(test.query_cw721_admin(), Some(admin));
    assert_eq!(test.query_contract_addr_length(), Some(20),);

    // migrate without changing code id
    let msg = MigrateMsg::WithUpdate {
        pauser: None,
        incoming_proxy: None,
        outgoing_proxy: None,
        cw721_base_code_id: None,
        cw721_admin: Some("".to_string()),
        contract_addr_length: None,
    };
    test.app
        .execute(
            test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER),
            WasmMsg::Migrate {
                contract_addr: test.ics721.to_string(),
                new_code_id: test.ics721_id,
                msg: to_json_binary(&msg).unwrap(),
            }
            .into(),
        )
        .unwrap();
    // assert migration worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(pauser, None);
    let proxy = test.query_outgoing_proxy();
    assert!(proxy.is_none());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, 12345678);
    assert_eq!(test.query_cw721_admin(), None);
    assert_eq!(test.query_contract_addr_length(), None);
}
