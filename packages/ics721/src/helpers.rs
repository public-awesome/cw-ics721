use cosmwasm_std::{
    from_json, instantiate2_address, to_json_binary, Addr, Binary, CodeInfoResponse, Deps,
    IbcPacket, StdResult, Storage, SubMsg, WasmMsg,
};
use serde::Deserialize;

use crate::{
    ibc::ACK_CALLBACK_REPLY_ID,
    state::{CONTRACT_ADDR_LENGTH, INCOMING_PROXY},
    ContractError,
};
use ics721_types::{
    ibc_types::NonFungibleTokenPacketData,
    types::{
        Ics721AckCallbackMsg, Ics721Callbacks, Ics721Memo, Ics721ReceiveCallbackMsg, Ics721Status,
        ReceiverExecuteMsg,
    },
};

/// Parse the memo field into the type we want
/// Ideally it would be `Ics721Memo` type or any type that extends it
fn parse_memo<T: for<'de> Deserialize<'de>>(memo: Option<String>) -> Option<T> {
    let binary = Binary::from_base64(memo?.as_str()).ok()?;
    from_json::<T>(&binary).ok()
}

/// Parse callback from the memo field
fn parse_callback(memo: Option<String>) -> Option<Ics721Callbacks> {
    parse_memo::<Ics721Memo>(memo)?.callbacks
}

// Create a subMsg that execute the callback on the sender callback
// we use a subMsg on error because we don't want to fail the whole tx
// if the callback fails
// if we were to fail the whole tx, the NFT would have been minted on
// the other chain while the NFT on this chain would not have been
// burned
pub(crate) fn ack_callback_msg(
    deps: Deps,
    status: Ics721Status,
    packet: NonFungibleTokenPacketData,
    nft_contract: String,
) -> Option<SubMsg> {
    // Get the callback object
    let callbacks = parse_callback(packet.memo.clone())?;

    // Validate the address
    let receiver = callbacks.ack_callback_addr.unwrap_or(packet.sender.clone());
    let contract_addr = deps.api.addr_validate(receiver.as_str()).ok()?.to_string();

    // Create the message we send to the contract
    // The status is the status we want to send back to the contract
    // The msg is the msg we forward from the sender
    let msg = to_json_binary(&ReceiverExecuteMsg::Ics721AckCallback(
        Ics721AckCallbackMsg {
            status,
            nft_contract,
            original_packet: packet,
            msg: callbacks.ack_callback_data?,
        },
    ))
    .ok()?;

    Some(SubMsg::reply_on_error(
        WasmMsg::Execute {
            contract_addr,
            msg,
            funds: vec![],
        },
        ACK_CALLBACK_REPLY_ID,
    ))
}

/// If there is an incoming proxy, let proxy validate the packet, in case it fails, we fail the transfer
/// This proxy for example whitelist channels that can send to this contract:
/// https://github.com/arkprotocol/cw-ics721-proxy/tree/main/contracts/cw-ics721-incoming-proxy-base
pub(crate) fn get_incoming_proxy_msg(
    storage: &dyn Storage,
    packet: IbcPacket,
    data: NonFungibleTokenPacketData,
) -> StdResult<Option<WasmMsg>> {
    match INCOMING_PROXY.load(storage).ok().flatten() {
        Some(incoming_proxy) => {
            let msg = to_json_binary(&ReceiverExecuteMsg::Ics721ReceivePacketMsg { packet, data })?;
            Ok(Some(WasmMsg::Execute {
                contract_addr: incoming_proxy.to_string(),
                msg,
                funds: vec![],
            }))
        }
        None => Ok(None),
    }
}

/// Get the msg and address from the memo field
/// if there is no receive callback returns None
pub(crate) fn get_receive_callback(
    packet: &NonFungibleTokenPacketData,
) -> Option<(Binary, Option<String>)> {
    let callbacks = parse_callback(packet.memo.clone())?;

    Some((
        callbacks.receive_callback_data?,
        callbacks.receive_callback_addr,
    ))
}

pub(crate) fn generate_receive_callback_msg(
    deps: Deps,
    packet: &NonFungibleTokenPacketData,
    receive_callback_data: Binary,
    receive_callback_addr: Option<String>,
    nft_contract: String,
) -> Option<WasmMsg> {
    let callback_receiver = receive_callback_addr.unwrap_or(packet.receiver.clone());
    let contract_addr = deps
        .api
        .addr_validate(callback_receiver.as_str())
        .ok()?
        .to_string();

    // Create the message we send to the contract
    // The status is the status we want to send back to the contract
    // The msg is the msg we forward from the sender
    let msg = to_json_binary(&ReceiverExecuteMsg::Ics721ReceiveCallback(
        Ics721ReceiveCallbackMsg {
            msg: receive_callback_data,
            nft_contract,
            original_packet: packet.clone(),
        },
    ))
    .ok()?;

    Some(WasmMsg::Execute {
        contract_addr,
        msg,
        funds: vec![],
    })
}

pub fn get_instantiate2_address(
    deps: Deps,
    creator: &str,
    salt: &[u8],
    code_id: u64,
) -> Result<Addr, ContractError> {
    // Get the canonical address of the contract creator
    let canonical_creator = deps.api.addr_canonicalize(creator)?;

    // get the checksum of the contract we're going to instantiate
    let CodeInfoResponse { checksum, .. } = deps.querier.query_wasm_code_info(code_id)?;

    let canonical_cw721_addr = instantiate2_address(&checksum, &canonical_creator, salt)?;
    if let Some(contract_addr_length) = CONTRACT_ADDR_LENGTH.may_load(deps.storage)? {
        let contract_addr_length = contract_addr_length as usize;
        Ok(deps
            .api
            .addr_humanize(&canonical_cw721_addr[..contract_addr_length].into())?)
    } else {
        Ok(deps.api.addr_humanize(&canonical_cw721_addr)?)
    }
}

mod test {
    #[test]
    fn test_parsing() {
        let memo = Some("some".to_string());
        let callbacks = super::parse_callback(memo);
        println!("{callbacks:?}")
    }
}
