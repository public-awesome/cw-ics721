use crate::error;
use std::str;
use cosmwasm_std::{
    to_binary, DepsMut, Empty, Env, IbcChannel, IbcChannelConnectMsg, ReplyOn, Response, SubMsg,
    WasmMsg,
};
use cw20_ics20::state::ChannelInfo;
use cw_storage_plus::{Item, Map};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use cw721_base_ibc::msg::InstantiateMsg;
use error::ContractError;
use sha2::{Digest, Sha256};

pub const ESCROW_CODE_ID: u64 = 9;
pub const INSTANTIATE_ESCROW721_REPLY_ID: u64 = 7;

const CONTRACT_NAME: &str = "crates.io:escrow721";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Serialize, Deserialize, Clone, PartialEq, JsonSchema, Debug, Default)]
pub struct Config {
    pub default_timeout: u64,
    pub cw721_ibc_code_id: u64,
    pub label: String,
}
pub const CONFIG: Item<Config> = Item::new("ics721_config");

/// static info on one channel that doesn't change
pub const CHANNEL_INFO: Map<&str, ChannelInfo> = Map::new("channel_info");

/// Indexed by (channel_id, contract_addr, token_id)
/// Keeps track of all NFTs that have passed through this channel.
pub const CHANNEL_STATE: Map<(&str, &str, &str), Empty> = Map::new("channel_state");

pub fn instantiate_escrow_contract(
    _deps: DepsMut,
    _env: Env,
    msg: IbcChannelConnectMsg,
) -> Result<Response, ContractError> {
    let escrow_name = construct_contract_name(_env.clone(), msg);
    let escrow_symbol = construct_contract_symbol(escrow_name.clone());
    let sub_msgs: Vec<SubMsg> = vec![SubMsg {
        msg: WasmMsg::Instantiate {
            code_id: ESCROW_CODE_ID,
            msg: to_binary(&InstantiateMsg {
                name: escrow_name.clone(),
                symbol: escrow_symbol.clone(),
                minter: _env.contract.address.to_string(),
            })?,
            funds: vec![],
            admin: Some(_env.contract.address.to_string()),
            label: String::from(escrow_name),
        }
        .into(),
        id: INSTANTIATE_ESCROW721_REPLY_ID,
        gas_limit: None,
        reply_on: ReplyOn::Success,
    }];

    Ok(Response::new()
        .add_attribute("action", "instantiate")
        .add_attribute("contract_name", CONTRACT_NAME)
        .add_attribute("contract_version", CONTRACT_VERSION)
        .add_attribute("sender", _env.contract.address.to_string())
        .add_submessages(sub_msgs))
}


fn construct_contract_name(_env: Env, msg: IbcChannelConnectMsg) -> String {
    // <chain_id>::<source_channel>/<source_port>:<dest_channel>/<dest_port>
    let channel: IbcChannel = msg.into();
    let chain_id = _env.block.chain_id;
    let source_channel = channel.endpoint.channel_id;
    let source_port = channel.endpoint.port_id;
    let dest_channel = channel.counterparty_endpoint.channel_id;
    let dest_port = channel.counterparty_endpoint.port_id;

    format!("{chain_id}::{source_channel}/{source_port}:{dest_channel}/{dest_port}")
}

fn construct_contract_symbol(contract_name: String) -> String {
    let hash_msg: Vec<u8> =  Sha256::digest(&contract_name).to_vec();
    let hash_msg_str = str::from_utf8(&hash_msg).unwrap();
    format!("ibc/{}", hash_msg_str)
}
