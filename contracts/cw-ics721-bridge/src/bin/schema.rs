use cosmwasm_schema::write_api;
use cw_ics721_bridge::msg::{InstantiateMsg, QueryMsg};
use ics721::msg::ExecuteMsg;

fn main() {
    write_api! {
        instantiate: InstantiateMsg,
        execute: ExecuteMsg,
        query: QueryMsg,
    }
}
