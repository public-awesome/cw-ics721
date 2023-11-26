use cosmwasm_std::{from_json, to_json_binary, Binary, Deps, SubMsg, WasmMsg};
use serde::Deserialize;

use crate::{
    ibc::{NonFungibleTokenPacketData, ACK_CALLBACK_REPLY_ID},
    token_types::ClassId,
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
            msg: callbacks.ack_callback_data?,
            original_packet: packet,
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

pub(crate) fn receive_callback_msg(
    deps: Deps,
    packet: NonFungibleTokenPacketData,
    local_class_id: ClassId,
) -> Option<WasmMsg> {
    // Get the callback object
    let callbacks = parse_callback(packet.memo.clone())?;

    // Validate the address
    let receiver = callbacks
        .receive_callback_addr
        .unwrap_or(packet.receiver.clone());
    let contract_addr = deps.api.addr_validate(receiver.as_str()).ok()?.to_string();

    // Create the message we send to the contract
    // The status is the status we want to send back to the contract
    // The msg is the msg we forward from the sender
    let msg = to_json_binary(&ReceiverExecuteMsg::Ics721ReceiveCallback(
        Ics721ReceiveCallbackMsg {
            msg: callbacks.receive_callback_data?,
            local_class_id,
            original_packet: packet,
        },
    ))
    .ok()?;

    Some(WasmMsg::Execute {
        contract_addr,
        msg,
        funds: vec![],
    })
}

mod test {
    #[test]
    fn test_parsing() {
        let memo = Some("some".to_string());
        let callbacks = super::parse_callback(memo);
        println!("{callbacks:?}")
    }
}
