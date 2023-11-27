use anyhow::Result;
use bech32::{decode, encode, FromBase32, ToBase32, Variant};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    from_json, instantiate2_address, to_json_binary, Addr, Api, Binary, CanonicalAddr, Deps,
    DepsMut, Empty, Env, GovMsg, IbcTimeout, IbcTimeoutBlock, MemoryStorage, MessageInfo,
    RecoverPubkeyError, Reply, Response, StdError, StdResult, Storage, VerificationError, WasmMsg,
};
use cw2::set_contract_version;
use cw721_base::msg::QueryMsg as Cw721QueryMsg;
use cw_cii::{Admin, ContractInstantiateInfo};
use cw_multi_test::{
    AddressGenerator, App, AppBuilder, BankKeeper, Contract, ContractWrapper, DistributionKeeper,
    Executor, FailingModule, IbcAcceptingModule, Router, StakeKeeper, WasmKeeper,
};
use cw_pause_once::PauseError;
use ics721::{
    execute::Ics721Execute,
    ibc::Ics721Ibc,
    msg::{CallbackMsg, ExecuteMsg, IbcOutgoingMsg, InstantiateMsg, MigrateMsg, QueryMsg},
    query::Ics721Query,
    state::CollectionData,
    token_types::{Class, ClassId, Token, TokenId, VoucherCreation},
};
use sg721::InstantiateMsg as Sg721InstantiateMsg;
use sg721_base::msg::{CollectionInfoResponse, QueryMsg as Sg721QueryMsg};
use sha2::{digest::Update, Digest, Sha256};

use crate::{state::SgCollectionData, ContractError, SgIcs721Contract};

const ICS721_CREATOR: &str = "ics721-creator";
const CONTRACT_NAME: &str = "crates.io:sg-ics721";
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
    fn new(
        proxy: bool,
        admin_and_pauser: Option<String>,
        cw721_code: Box<dyn Contract<Empty>>,
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
                    cw721_base_code_id: source_cw721_id,
                    proxy: proxy.clone(),
                    pauser: admin_and_pauser
                        .clone()
                        .map(|p| app.api().addr_make(&p).to_string()),
                },
                &[],
                "sg-ics721",
                admin_and_pauser
                    .clone()
                    .map(|p| app.api().addr_make(&p).to_string()),
            )
            .unwrap();

        // minter of sg721-base must be a contract!
        let source_cw721_owner = ics721.clone();
        let source_cw721 = app
            .instantiate_contract(
                source_cw721_id,
                source_cw721_owner.clone(),
                &Sg721InstantiateMsg {
                    name: "name".to_string(),
                    symbol: "symbol".to_string(),
                    minter: source_cw721_owner.to_string(),
                    collection_info: sg721::CollectionInfo {
                        creator: source_cw721_owner.to_string(),
                        description: "".to_string(),
                        image: "https://arkprotocol.io".to_string(),
                        external_link: None,
                        explicit_content: None,
                        start_trading_time: None,
                        royalty_info: None,
                    },
                },
                &[],
                "cw721-base",
                None,
            )
            .unwrap();

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
                self.source_cw721_owner.clone(),
                self.source_cw721.clone(),
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

fn sg721_base_contract() -> Box<dyn Contract<Empty>> {
    // sg721_base's execute and instantiate function deals Response<StargazeMsgWrapper>
    // but App multi test deals Response<Empty>
    // so we need to wrap sg721_base's execute and instantiate function
    fn exececute_fn(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: sg721::ExecuteMsg<Option<Empty>, Empty>,
    ) -> Result<Response, sg721_base::ContractError> {
        sg721_base::entry::execute(deps, env, info, msg).map(|_| Response::default())
    }
    fn instantiate_fn(
        deps: DepsMut,
        env: Env,
        info: MessageInfo,
        msg: sg721::InstantiateMsg,
    ) -> Result<Response, sg721_base::ContractError> {
        sg721_base::entry::instantiate(deps, env, info, msg).map(|_| Response::default())
    }
    let contract = ContractWrapper::new(exececute_fn, instantiate_fn, sg721_base::entry::query);
    Box::new(contract)
}

fn ics721_contract() -> Box<dyn Contract<Empty>> {
    // need to wrap method in function for testing
    fn ibc_reply(deps: DepsMut, env: Env, reply: Reply) -> Result<Response, ContractError> {
        SgIcs721Contract::default().reply(deps, env, reply)
    }

    let contract = ContractWrapper::new(execute, instantiate, query)
        .with_migrate(migrate)
        .with_reply(ibc_reply);
    Box::new(contract)
}

fn proxy_contract() -> Box<dyn Contract<Empty>> {
    let execute_fn = cw721_rate_limited_proxy::contract::execute::<Empty>;
    let instatiate_fn = cw721_rate_limited_proxy::contract::instantiate::<Empty>;
    let contract = ContractWrapper::new(
        execute_fn,
        instatiate_fn,
        cw721_rate_limited_proxy::contract::query,
    );
    Box::new(contract)
}

#[test]
fn test_instantiate() {
    let mut test = Test::new(false, None, sg721_base_contract());

    // check stores are properly initialized
    let cw721_id = test.query_cw721_id();
    assert_eq!(cw721_id, test.source_cw721_id);
    let nft_contracts: Vec<(String, Addr)> = test.query_nft_contracts();
    assert_eq!(nft_contracts, Vec::<(String, Addr)>::new());
    let outgoing_channels = test.query_outgoing_channels();
    assert_eq!(outgoing_channels, []);
    let incoming_channels = test.query_incoming_channels();
    assert_eq!(incoming_channels, []);
}

#[test]
fn test_do_instantiate_and_mint_weird_data() {
    let mut test = Test::new(false, None, sg721_base_contract());
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
                            // data comes from source chain, so it can't be SgCollectionData
                            to_json_binary(&CollectionData {
                                // incoming collection data from source chain
                                owner: Some(
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
    // test case: instantiate cw721 with no ClassData (without owner, name, and symbol)
    {
        let mut test = Test::new(false, None, sg721_base_contract());
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
        assert_eq!(nft_contracts[0].0, class_id.to_string());
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
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &Cw721QueryMsg::<Empty>::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::ContractInfoResponse {
                name: class_id.to_string(),   // name is set to class_id
                symbol: class_id.to_string()  // symbol is set to class_id
            }
        );

        // check collection info is properly set
        let collection_info: CollectionInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft_contract.clone(), &Sg721QueryMsg::CollectionInfo {})
            .unwrap();

        assert_eq!(
            collection_info,
            CollectionInfoResponse {
                // creator of ics721 contract is also creator of collection, since no owner in ClassData provided
                creator: test.app.api().addr_make(ICS721_CREATOR).to_string(),
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
                nft_contract.clone(),
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
                nft_contract.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
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
                &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
                    recipient: nft_contract.to_string(),
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();
        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
                &cw721::Cw721QueryMsg::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        assert_eq!(base_owner, owner);
    }
    // test case: instantiate cw721 with ClassData containing owner, name, and symbol
    {
        let mut test = Test::new(false, None, sg721_base_contract());
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
                                // data comes from source chain, so it can't be SgCollectionData
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
        assert_eq!(nft_contracts[0].0, class_id.to_string());
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
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &Cw721QueryMsg::<Empty>::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::ContractInfoResponse {
                name: "ark".to_string(),
                symbol: "protocol".to_string()
            }
        );

        // check collection info is properly set
        let collection_info: CollectionInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft_contract.clone(), &Sg721QueryMsg::CollectionInfo {})
            .unwrap();

        assert_eq!(
            collection_info,
            CollectionInfoResponse {
                // creator based on owner from collection in soure chain
                creator: test.app.api().addr_make(ICS721_CREATOR).to_string(),
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
                nft_contract.clone(),
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
                nft_contract.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
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
                &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
                    recipient: nft_contract.to_string(),
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();
        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
                &cw721::Cw721QueryMsg::OwnerOf {
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
        let mut test = Test::new(false, None, sg721_base_contract());
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
        assert_eq!(nft_contracts[0].0, class_id.to_string());
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
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &Cw721QueryMsg::<Empty>::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::ContractInfoResponse {
                name: class_id.to_string(),
                symbol: class_id.to_string()
            }
        );

        // check collection info is properly set
        let collection_info: CollectionInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft_contract.clone(), &Sg721QueryMsg::CollectionInfo {})
            .unwrap();

        assert_eq!(
            collection_info,
            CollectionInfoResponse {
                // creator of ics721 contract is creator of nft contract, since no owner in ClassData provided
                creator: test.app.api().addr_make(ICS721_CREATOR).to_string(),
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
                nft_contract.clone(),
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
                nft_contract.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
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
                &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
                    recipient: nft_contract.to_string(), // new owner
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();

        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
                &cw721::Cw721QueryMsg::OwnerOf {
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
        let mut test = Test::new(false, None, sg721_base_contract());
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
        assert_eq!(nft_contracts[0].0, class_id.to_string());
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
        let contract_info: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract.clone(),
                &Cw721QueryMsg::<Empty>::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info,
            cw721::ContractInfoResponse {
                name: "collection-name".to_string(),
                symbol: "collection-symbol".to_string()
            }
        );

        // check collection info is properly set
        let collection_info: CollectionInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft_contract.clone(), &Sg721QueryMsg::CollectionInfo {})
            .unwrap();

        assert_eq!(
            collection_info,
            CollectionInfoResponse {
                // creator of ics721 contract is creator of nft contract, since no owner in ClassData provided
                creator: test.app.api().addr_make(ICS721_CREATOR).to_string(),
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
                nft_contract.clone(),
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
                nft_contract.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
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
                &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
                    recipient: nft_contract.to_string(), // new owner
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id.to_string(),
                },
            )
            .unwrap();

        assert_eq!(owner.owner, nft_contract.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract,
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
fn test_do_instantiate_and_mint_2_different_collections() {
    // test case: instantiate two cw721 contracts with different class id and make sure instantiate2 creates 2 different, predictable contracts
    {
        let mut test = Test::new(false, None, sg721_base_contract());
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
        assert_eq!(nft_contracts[0].0, class_id_1.to_string());
        assert_eq!(nft_contracts[1].0, class_id_2.to_string());
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
        let contract_info_1: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1.clone(),
                &Cw721QueryMsg::<Empty>::ContractInfo {},
            )
            .unwrap();
        let contract_info_2: cw721::ContractInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2.clone(),
                &Cw721QueryMsg::<Empty>::ContractInfo {},
            )
            .unwrap();
        assert_eq!(
            contract_info_1,
            cw721::ContractInfoResponse {
                name: class_id_1.to_string(),   // name is set to class_id
                symbol: class_id_1.to_string()  // symbol is set to class_id
            }
        );
        assert_eq!(
            contract_info_2,
            cw721::ContractInfoResponse {
                name: class_id_2.to_string(),   // name is set to class_id
                symbol: class_id_2.to_string()  // symbol is set to class_id
            }
        );

        // check collection info is properly set
        let collection_info_1: CollectionInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft_contract_1.clone(), &Sg721QueryMsg::CollectionInfo {})
            .unwrap();
        let collection_info_2: CollectionInfoResponse = test
            .app
            .wrap()
            .query_wasm_smart(nft_contract_2.clone(), &Sg721QueryMsg::CollectionInfo {})
            .unwrap();
        assert_eq!(
            collection_info_1,
            CollectionInfoResponse {
                // creator of ics721 contract is also creator of collection, since no owner in ClassData provided
                creator: test.app.api().addr_make(ICS721_CREATOR).to_string(),
                description: "".to_string(),
                image: "https://arkprotocol.io".to_string(),
                external_link: None,
                explicit_content: None,
                start_trading_time: None,
                royalty_info: None,
            }
        );
        assert_eq!(
            collection_info_2,
            CollectionInfoResponse {
                // creator of ics721 contract is also creator of collection, since no owner in ClassData provided
                creator: test.app.api().addr_make(ICS721_CREATOR).to_string(),
                description: "".to_string(),
                image: "https://arkprotocol.io".to_string(),
                external_link: None,
                explicit_content: None,
                start_trading_time: None,
                royalty_info: None,
            }
        );

        // Check that token_uri was set properly.
        let token_info_1_1: cw721::NftInfoResponse<Empty> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        let token_info_2_1: cw721::NftInfoResponse<Empty> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
                    token_id: "1".to_string(),
                },
            )
            .unwrap();
        assert_eq!(
            token_info_1_1.token_uri,
            Some("https://moonphase.is/image.svg".to_string())
        );
        assert_eq!(token_info_2_1.token_uri, Some("https://mr.t".to_string()));
        let token_info_1_2: cw721::NftInfoResponse<Empty> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
                    token_id: "2".to_string(),
                },
            )
            .unwrap();
        let token_info_2_2: cw721::NftInfoResponse<Empty> = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2.clone(),
                &cw721::Cw721QueryMsg::NftInfo {
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
                &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
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
                &cw721_base::msg::ExecuteMsg::<Empty, Empty>::TransferNft {
                    recipient: nft_contract_2.to_string(),
                    token_id: "1".to_string(),
                },
                &[],
            )
            .unwrap();

        // ics721 owner query and check nft contract owns it
        let owner_1: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721.clone(),
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id_1.to_string(),
                },
            )
            .unwrap();
        let owner_2: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                test.ics721,
                &QueryMsg::Owner {
                    token_id: "1".to_string(),
                    class_id: class_id_2.to_string(),
                },
            )
            .unwrap();
        assert_eq!(owner_1.owner, nft_contract_1.to_string());
        assert_eq!(owner_2.owner, nft_contract_2.to_string());

        // check cw721 owner query matches ics721 owner query
        let base_owner_1: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_1,
                &cw721::Cw721QueryMsg::OwnerOf {
                    token_id: "1".to_string(),
                    include_expired: None,
                },
            )
            .unwrap();
        let base_owner_2: cw721::OwnerOfResponse = test
            .app
            .wrap()
            .query_wasm_smart(
                nft_contract_2,
                &cw721::Cw721QueryMsg::OwnerOf {
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
    let mut test = Test::new(false, None, sg721_base_contract());
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
                            // data comes from source chain, so it can't be SgCollectionData
                            // owner as defined by collection in source chain
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
    assert_eq!(class_id_to_nft_contract[0].0, class_id.to_string());

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
        .query_wasm_smart(
            test.ics721,
            &QueryMsg::NftContract {
                class_id: class_id.to_string(),
            },
        )
        .unwrap();

    // check collection info is properly set
    let collection_info: CollectionInfoResponse = test
        .app
        .wrap()
        .query_wasm_smart(nft_contract.clone(), &Sg721QueryMsg::CollectionInfo {})
        .unwrap();
    assert_eq!(
        collection_info,
        CollectionInfoResponse {
            creator: test.app.api().addr_make(ICS721_CREATOR).to_string(),
            description: "".to_string(),
            image: "https://arkprotocol.io".to_string(),
            external_link: None,
            explicit_content: None,
            start_trading_time: None,
            royalty_info: None,
        }
    );

    // Make sure we have our tokens.
    let tokens: cw721::TokensResponse = test
        .app
        .wrap()
        .query_wasm_smart(
            nft_contract,
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
    let mut test = Test::new(false, None, sg721_base_contract());
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

/// Tests that we can not proxy NFTs if no proxy is configured.
#[test]
fn test_no_proxy_unauthorized() {
    let mut test = Test::new(false, None, sg721_base_contract());
    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("proxy"),
            test.ics721,
            &ExecuteMsg::ReceiveProxyNft {
                eyeball: "nft".to_string(),
                msg: cw721::Cw721ReceiveMsg {
                    sender: test.app.api().addr_make(NFT_OWNER_TARGET_CHAIN).to_string(),
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
    let mut test = Test::new(true, None, sg721_base_contract());
    let proxy_address: Option<Addr> = test
        .app
        .wrap()
        .query_wasm_smart(&test.ics721, &QueryMsg::Proxy {})
        .unwrap();
    // check proxy is set
    let proxy_address = proxy_address.expect("expected a proxy");

    // create collection and mint NFT for sending to proxy
    let source_cw721_id = test.app.store_code(sg721_base_contract());
    let source_cw721 = test
        .app
        .instantiate_contract(
            source_cw721_id,
            test.ics721.clone(), // sg721 contract can only be instantiated by a contract, not user (unauthorized)
            &sg721::InstantiateMsg {
                name: "token".to_string(),
                symbol: "nonfungible".to_string(),
                minter: test
                    .app
                    .api()
                    .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                    .to_string(),
                collection_info: sg721::CollectionInfo {
                    creator: test
                        .app
                        .api()
                        .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                        .to_string(),
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
    // simplify: instead of `send_nft` to proxy, and proxy transfer NFT to ics721 and call receiveproy,
    // here it is directly transfer to ics721 and then call receiveproxy
    test.app
        .execute_contract(
            test.app.api().addr_make(COLLECTION_OWNER_SOURCE_CHAIN),
            source_cw721.clone(),
            &cw721_base::ExecuteMsg::<Empty, Empty>::Mint {
                token_id: "1".to_string(),
                owner: test.ics721.to_string(),
                token_uri: None,
                extension: Empty::default(),
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
            &ExecuteMsg::ReceiveProxyNft {
                eyeball: source_cw721.into_string(),
                msg: cw721::Cw721ReceiveMsg {
                    sender: test
                        .app
                        .api()
                        .addr_make(COLLECTION_OWNER_SOURCE_CHAIN)
                        .to_string(),
                    token_id: "1".to_string(),
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
                },
            },
            &[],
        )
        .unwrap();
}

#[test]
fn test_receive_nft() {
    // test case: receive nft from sg721-base
    {
        let mut test = Test::new(false, None, sg721_base_contract());
        // simplify: mint and escrowed/owned by ics721, as a precondition for receive nft
        let token_id = test.execute_cw721_mint(test.ics721.clone()).unwrap();
        // ics721 receives NFT from sender/collection contract,
        // if - and only if - nft is escrowed by ics721, ics721 will interchain transfer NFT!
        // otherwise proxy is unauthorised to transfer NFT
        let res = test
            .app
            .execute_contract(
                test.source_cw721.clone(),
                test.ics721.clone(),
                &ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
                    sender: test.source_cw721_owner.to_string(),
                    token_id: token_id.clone(),
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
        let expected_collection_data = to_json_binary(&SgCollectionData {
            owner: Some(
                // collection data from source chain
                test.source_cw721_owner.to_string(),
            ),
            contract_info: Some(expected_contract_info),
            name: "name".to_string(),
            symbol: "symbol".to_string(),
            num_tokens: Some(1),
            collection_info: Some(CollectionInfoResponse {
                creator: test.ics721.to_string(),
                description: "".to_string(),
                image: "https://arkprotocol.io".to_string(),
                external_link: None,
                explicit_content: None,
                start_trading_time: None,
                royalty_info: None,
            }),
        })
        .unwrap();
        assert_eq!(
            class_data_attribute.value,
            format!("{:?}", expected_collection_data)
        );
    }
}

/// In case proxy for ICS721 is defined, ICS721 only accepts receival from proxy - not from nft contract!
#[test]
fn test_no_receive_with_proxy() {
    let mut test = Test::new(true, None, sg721_base_contract());
    // unauthorized to receive nft from nft contract
    let err: ContractError = test
        .app
        .execute_contract(
            test.app.api().addr_make("cw721"),
            test.ics721,
            &ExecuteMsg::ReceiveNft(cw721::Cw721ReceiveMsg {
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
        Some(ICS721_ADMIN_AND_PAUSER.to_string()),
        sg721_base_contract(),
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
    let err = test.pause_ics721_should_fail(test.app.api().addr_make("mr-t").as_str());
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
        Some(ICS721_ADMIN_AND_PAUSER.to_string()),
        sg721_base_contract(),
    );
    // assert instantiation worked
    let (_, pauser) = test.query_pause_info();
    assert_eq!(
        pauser,
        Some(test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER))
    );
    let proxy = test.query_proxy();
    assert!(proxy.is_some());
    let cw721_code_id = test.query_cw721_id();
    assert_eq!(cw721_code_id, test.source_cw721_id);

    // migrate changes
    test.app
        .execute(
            test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER),
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
            test.app.api().addr_make(ICS721_ADMIN_AND_PAUSER),
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
