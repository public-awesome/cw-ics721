use std::env::current_dir;
use std::fs::create_dir_all;

// TODO: Readd these once more finalised, export_schema, schema_for
use cosmwasm_schema::remove_schemas;

// use cw721_ics::msg::{CountResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
// use cw721_ics::state::State;

fn main() {
    let mut out_dir = current_dir().unwrap();
    out_dir.push("schema");
    create_dir_all(&out_dir).unwrap();
    remove_schemas(&out_dir).unwrap();

    // export_schema(&schema_for!(InstantiateMsg), &out_dir);
    // export_schema(&schema_for!(ExecuteMsg), &out_dir);
    // export_schema(&schema_for!(QueryMsg), &out_dir);
    // export_schema(&schema_for!(State), &out_dir);
    // export_schema(&schema_for!(CountResponse), &out_dir);
}
