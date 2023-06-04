use cosmwasm_std::{from_binary, to_binary, Binary, CosmosMsg, Deps, SubMsg, WasmMsg};
use ics721::{Ics721Callbacks, Ics721Memo, Ics721ReceiveMsg, Ics721Status};
use serde::Deserialize;

use crate::ibc::ACK_CALLBACK_REPLY_ID;

/// Parse the memo field into the type we want
/// Ideally it would be `Ics721Memo` type or any type that extends it
fn parse_memo<T: for<'de> Deserialize<'de>>(memo: Option<String>) -> Option<T> {
    let binary = Binary::from_base64(memo?.as_str()).ok()?;
    from_binary::<T>(&binary).ok()
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
    memo: Option<String>,
    status: Ics721Status,
    sender: String,
) -> Option<SubMsg> {
    // Get the callback object
    let callbacks = parse_callback(memo)?;

    // Create the message we send to the contract
    // The status is the status we want to send back to the contract
    // The msg is the msg we forward from the sender
    let msg = to_binary(&Ics721ReceiveMsg {
        status,
        msg: callbacks.src_callback_msg?,
    })
    .ok()?;

    // Validate the address
    let contract_addr = deps.api.addr_validate(sender.as_str()).ok()?.to_string();

    Some(SubMsg::reply_on_error(
        WasmMsg::Execute {
            contract_addr,
            msg,
            funds: vec![],
        },
        ACK_CALLBACK_REPLY_ID,
    ))
}

pub(crate) fn receive_callback_msg(
    deps: Deps,
    memo: Option<String>,
    receiver: String,
) -> Option<CosmosMsg> {
    // Get the callback object
    let callbacks = parse_callback(memo)?;

    // Create the message we send to the contract
    // The status is the status we want to send back to the contract
    // The msg is the msg we forward from the sender
    let msg = to_binary(&Ics721ReceiveMsg {
        status: Ics721Status::Success,
        msg: callbacks.dest_callback_msg?,
    })
    .ok()?;

    // Validate the address
    let contract_addr = deps.api.addr_validate(receiver.as_str()).ok()?.to_string();

    Some(
        WasmMsg::Execute {
            contract_addr,
            msg,
            funds: vec![],
        }
        .into(),
    )
}
