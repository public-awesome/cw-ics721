use cosmwasm_std::{Addr, Empty};
use cw_multi_test::{App, Contract, ContractWrapper, Executor};

use crate::msg::{ChannelInfoResponse, InstantiateMsg, QueryMsg};

const COMMUNITY_POOL: &str = "community_pool";

fn cw721_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        cw721_ics::contract::execute,
        cw721_ics::contract::instantiate,
        cw721_ics::contract::query,
    );
    Box::new(contract)
}

fn escrow_contract() -> Box<dyn Contract<Empty>> {
    let contract = ContractWrapper::new(
        ics721_escrow::contract::execute,
        ics721_escrow::contract::instantiate,
        ics721_escrow::contract::query,
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

#[test]
fn test_instantiate() {
    let mut app = App::default();

    let cw721_id = app.store_code(cw721_contract());
    let escrow_id = app.store_code(escrow_contract());
    let bridge_id = app.store_code(bridge_contract());

    let bridge = app
        .instantiate_contract(
            bridge_id,
            Addr::unchecked(COMMUNITY_POOL),
            &InstantiateMsg {
                cw721_ics_code_id: cw721_id,
                escrow_code_id: escrow_id,
            },
            &[],
            "cw-ics721-bridge",
            None,
        )
        .unwrap();

    let channels: Vec<ChannelInfoResponse> = app
        .wrap()
        .query_wasm_smart(
            bridge,
            &QueryMsg::ListChannels {
                start_after: None,
                limit: None,
            },
        )
        .unwrap();

    assert!(channels.is_empty())
}
