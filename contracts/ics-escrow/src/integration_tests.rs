use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use cosmwasm_std::{Addr, Empty};
use cw721::OwnerOfResponse;
use cw_multi_test::{App, AppResponse, Contract, ContractWrapper, Executor};

/// Name of the NFT collection
const NAME: &str = "Interchain Nifties";
/// Symbol ticker for the NFT collection
const SYMBOL: &str = "ICSNFT";
/// In reality the minter will be the ICS contract
const ADMIN: &str = "stars1minter";

/// Other addresses representing normal users
const ADDR1: &str = "stars1yyy";

fn contract_escrow() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        crate::contract::execute,
        crate::contract::instantiate,
        crate::contract::query,
    );
    Box::new(contract)
}

fn contract_cw721() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_base::entry::execute,
        cw721_base::entry::instantiate,
        cw721_base::entry::query,
    );
    Box::new(contract)
}

fn instantiate_escrow(app: &mut App, admin_address: &str) -> Addr {
    let id = app.store_code(contract_escrow());
    app.instantiate_contract(
        id,
        Addr::unchecked(ADMIN),
        &InstantiateMsg {
            admin_address: admin_address.to_string(),
        },
        &[],
        "escrow",
        None,
    )
    .unwrap()
}

fn instantiate_cw721(app: &mut App) -> Addr {
    let id = app.store_code(contract_cw721());
    app.instantiate_contract(
        id,
        Addr::unchecked(ADMIN),
        &cw721_base::msg::InstantiateMsg {
            name: NAME.to_string(),
            symbol: SYMBOL.to_string(),
            minter: ADMIN.to_string(),
        },
        &[],
        "cw721",
        None,
    )
    .unwrap()
}

fn mint(
    app: &mut App,
    addr: Addr,
    sender: &str,
    token_id: &str,
    owner: &str,
    token_uri: Option<String>,
) -> anyhow::Result<AppResponse> {
    let mint_msg = cw721_base::MintMsg::<Empty> {
        token_id: token_id.to_string(),
        owner: owner.to_string(),
        token_uri,
        extension: Empty {},
    };
    let msg = cw721_base::ExecuteMsg::Mint(mint_msg);

    app.execute_contract(Addr::unchecked(sender), addr, &msg, &[])
}

fn transfer_nft(
    app: &mut App,
    addr: Addr,
    sender: &str,
    recipient: &str,
    token_id: &str,
) -> anyhow::Result<AppResponse> {
    let msg = cw721_base::ExecuteMsg::<Empty>::TransferNft {
        recipient: recipient.to_string(),
        token_id: token_id.to_string(),
    };
    app.execute_contract(Addr::unchecked(sender), addr, &msg, &[])
}

fn withdraw(
    app: &mut App,
    addr: Addr,
    sender: &str,
    class_uri: &str,
    token_id: &str,
    receiver: &str,
) -> anyhow::Result<AppResponse> {
    let msg = ExecuteMsg::Withdraw {
        class_uri: class_uri.to_string(),
        token_id: token_id.to_string(),
        receiver: receiver.to_string(),
    };
    app.execute_contract(Addr::unchecked(sender), addr, &msg, &[])
}

fn owner_of(app: &mut App, addr: Addr, token_id: &str) -> OwnerOfResponse {
    let msg = cw721_base::QueryMsg::OwnerOf {
        token_id: token_id.to_string(),
        include_expired: None,
    };
    app.wrap().query_wasm_smart(addr, &msg).unwrap()
}

fn admin_address(app: &mut App, addr: Addr) -> Addr {
    let msg = QueryMsg::AdminAddress {};
    app.wrap().query_wasm_smart(addr, &msg).unwrap()
}

#[test]
fn test_instantiate() {
    let mut app = App::default();
    let escrow = instantiate_escrow(&mut app, ADMIN);
    let admin_address = admin_address(&mut app, escrow);
    assert_eq!(admin_address, Addr::unchecked(ADMIN))
}

#[test]
fn test_withdraw_valid() {
    let mut app = App::default();
    let escrow = instantiate_escrow(&mut app, ADMIN);
    let cw721 = instantiate_cw721(&mut app);

    // Mint NFT to ADDR1, we could mint directly to escrow but
    // I want to follow the critical path. We can pretend ADDR1
    // is our bridge.
    let token_id = "1";
    mint(&mut app, cw721.clone(), ADMIN, token_id, ADDR1, None).unwrap();

    // Sanity check ADDR1 is owner of token_id 1
    let resp = owner_of(&mut app, cw721.clone(), token_id);
    assert_eq!(resp.owner, ADDR1.to_string());

    // Transfer NFT to our escrow, no logic happens on the escrow
    // purely a transfer on cw721
    transfer_nft(&mut app, cw721.clone(), ADDR1, escrow.as_str(), token_id).unwrap();

    // Sanity check escrow is owner of token_id 1
    let resp = owner_of(&mut app, cw721.clone(), token_id);
    assert_eq!(resp.owner, escrow.to_string());

    // Attempt to withdraw the NFT back to ADDR1
    withdraw(&mut app, escrow, ADMIN, cw721.as_str(), token_id, ADDR1).unwrap();

    // Check that ADDR1 is now the owner of token_id 1
    let resp = owner_of(&mut app, cw721, token_id);
    assert_eq!(resp.owner, ADDR1.to_string());
}

#[test]
#[should_panic(expected = "Unauthorized")]
fn test_withdraw_non_admin() {
    let mut app = App::default();
    let escrow = instantiate_escrow(&mut app, ADMIN);
    let cw721 = instantiate_cw721(&mut app);

    // Mint NFT to ADDR1, we could mint directly to escrow but
    // I want to follow the critical path. We can pretend ADDR1
    // is our bridge.
    let token_id = "1";
    mint(&mut app, cw721.clone(), ADMIN, token_id, ADDR1, None).unwrap();

    // Sanity check ADDR1 is owner of token_id 1
    let resp = owner_of(&mut app, cw721.clone(), token_id);
    assert_eq!(resp.owner, ADDR1.to_string());

    // Transfer NFT to our escrow, no logic happens on the escrow
    // purely a transfer on cw721
    transfer_nft(&mut app, cw721.clone(), ADDR1, escrow.as_str(), token_id).unwrap();

    // Try and withdraw as non admin, will fail
    withdraw(&mut app, escrow, ADDR1, cw721.as_str(), token_id, ADDR1).unwrap();
}
#[test]
#[should_panic(expected = "not found")]
fn test_withdraw_no_nft_for_given_contract_address() {
    let mut app = App::default();
    let escrow = instantiate_escrow(&mut app, ADMIN);
    let cw721 = instantiate_cw721(&mut app);

    // Try and withdraw without ever having sent an NFT into
    let token_id = "1";
    withdraw(&mut app, escrow, ADMIN, cw721.as_str(), token_id, ADDR1).unwrap();
}

#[test]
#[should_panic(expected = "not found")]
fn test_withdraw_no_nft_with_given_token_id() {
    let mut app = App::default();
    let escrow = instantiate_escrow(&mut app, ADMIN);
    let cw721 = instantiate_cw721(&mut app);

    // Mint NFT to ADDR1, we could mint directly to escrow but
    // I want to follow the critical path. We can pretend ADDR1
    // is our bridge.
    let token_id = "1";
    mint(&mut app, cw721.clone(), ADMIN, token_id, ADDR1, None).unwrap();

    // Sanity check ADDR1 is owner of token_id 1
    let resp = owner_of(&mut app, cw721.clone(), token_id);
    assert_eq!(resp.owner, ADDR1.to_string());

    // Transfer NFT to our escrow, no logic happens on the escrow
    // purely a transfer on cw721
    transfer_nft(&mut app, cw721.clone(), ADDR1, escrow.as_str(), token_id).unwrap();

    // Try and withdraw with invalid token_id
    withdraw(&mut app, escrow, ADMIN, cw721.as_str(), "2", ADDR1).unwrap();
}
