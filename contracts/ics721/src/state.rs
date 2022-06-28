use crate::error;
use cosmwasm_std::{
    to_binary, Addr, Empty, Env, IbcChannel, IbcChannelConnectMsg, ReplyOn, Response, Storage,
    SubMsg, WasmMsg,
};
use cw20_ics20::state::ChannelInfo;
use cw721_base_ibc::msg::InstantiateMsg;
use cw_storage_plus::{Item, Map};
use error::ContractError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::str;

pub const INSTANTIATE_ESCROW721_REPLY_ID: u64 = 1;
pub const ESCROW_LOAD_CONTRACT_ID: u64 = 2;

const CONTRACT_NAME: &str = "crates.io:escrow721";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

pub const ESCROW_STORAGE_MAP: Map<&str, EscrowMetadata> = Map::new("escrow_storage_map");
pub const CONFIG: Item<Config> = Item::new("ics721_config");

/// static info on one channel that doesn't change
pub const CHANNEL_INFO: Map<&str, ChannelInfo> = Map::new("channel_info");

/// Indexed by (channel_id, contract_addr, token_id)
/// Keeps track of all NFTs that have passed through this channel.
pub const CHANNEL_STATE: Map<(&str, &str, &str), Empty> = Map::new("channel_state");

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Config {
    pub default_timeout: u64,
    pub cw721_ibc_code_id: u64,
    pub label: String,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug)]
pub struct EscrowMetadata {
    pub contract_address: Addr,
    pub is_active: bool,
}

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct CurrentEscrowData {
    pub escrow_name: String,
}
pub const CURRENT_ESCROW_DATA: Item<CurrentEscrowData> = Item::new("current_escrow_data");

pub fn instantiate_escrow_contract(
    storage: &mut dyn Storage,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<Response, ContractError> {
    let escrow_name = construct_escrow_name(_env.clone(), msg);
    let escrow_symbol = construct_escrow_symbol(escrow_name.clone());
    let mut response = construct_response(_env.clone().contract.address);

    let already_exists = ESCROW_STORAGE_MAP.may_load(storage, &escrow_name).unwrap();
    match already_exists {
        Some(escrow_metadata) => set_escrow_is_active(storage, escrow_name, escrow_metadata),
        None => {
            response = add_submsg_to_response(storage, _env, response, escrow_name, escrow_symbol);
        }
    }
    Ok(response)
}

fn construct_response(contract_address: Addr) -> Response {
    Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("sender", contract_address.to_string())
}

fn set_escrow_is_active(
    storage: &mut dyn Storage,
    escrow_name: String,
    escrow_metadata: EscrowMetadata,
) {
    match escrow_metadata.is_active {
        true => {}
        false => {
            ESCROW_STORAGE_MAP
                .save(
                    storage,
                    &escrow_name,
                    &EscrowMetadata {
                        contract_address: escrow_metadata.contract_address,
                        is_active: true,
                    },
                )
                .unwrap();
        }
    }
}

fn add_submsg_to_response(
    storage: &mut dyn Storage,
    _env: Env,
    response: Response,
    escrow_name: String,
    escrow_symbol: String,
) -> Response {
    CURRENT_ESCROW_DATA
        .save(
            storage,
            &CurrentEscrowData {
                escrow_name: escrow_name.clone(),
            },
        )
        .unwrap();
    let sub_msg = get_submsg(_env, escrow_name, escrow_symbol);
    response.add_submessage(sub_msg)
}

fn get_submsg(_env: Env, escrow_name: String, escrow_symbol: String) -> SubMsg {
    SubMsg {
        msg: get_wasm_msg(_env, escrow_name, escrow_symbol).into(),
        id: INSTANTIATE_ESCROW721_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }
}
fn get_wasm_msg(_env: Env, escrow_name: String, escrow_symbol: String) -> WasmMsg {
    WasmMsg::Instantiate {
        code_id: ESCROW_LOAD_CONTRACT_ID,
        msg: to_binary(&InstantiateMsg {
            name: escrow_name.clone(),
            symbol: escrow_symbol,
            minter: _env.contract.address.to_string(),
        })
        .unwrap(),
        funds: vec![],
        admin: Some(_env.contract.address.to_string()),
        label: escrow_name,
    }
}

pub fn construct_escrow_name(_env: Env, msg: IbcChannelConnectMsg) -> String {
    // <source_channel>/<source_port>:<dest_channel>/<dest_port>
    let channel: IbcChannel = msg.into();
    let source_channel = channel.endpoint.channel_id;
    let source_port = channel.endpoint.port_id;
    let dest_channel = channel.counterparty_endpoint.channel_id;
    let dest_port = channel.counterparty_endpoint.port_id;

    format!("{source_channel}/{source_port}:{dest_channel}/{dest_port}")
}

fn construct_escrow_symbol(contract_name: String) -> String {
    let hash_encode = Sha256::digest(contract_name.as_bytes());
    let hex_string = hex::encode(hash_encode);
    format!("ibc/{}", hex_string)
}
