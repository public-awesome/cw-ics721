use cosmwasm_schema::{cw_serde, schemars::JsonSchema};
use cosmwasm_std::{Binary, IbcPacket};
use serde::{Deserialize, Serialize};

use crate::ibc::NonFungibleTokenPacketData;

#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[schemars(crate = "cosmwasm_schema::schemars")]
#[serde(crate = "cosmwasm_schema::serde")]
pub struct Ics721Memo {
    pub callbacks: Option<Ics721Callbacks>,
}

/// The format we expect for the memo field on a send
#[cw_serde]
pub struct Ics721Callbacks {
    /// Data to pass with a callback on source side (status update)
    /// Note - If this field is empty, no callback will be sent
    pub ack_callback_data: Option<Binary>,
    /// The address that will receive the callback message
    /// Defaults to the sender address
    pub ack_callback_addr: Option<String>,
    /// Data to pass with a callback on the destination side (ReceiveNftIcs721)
    /// Note - If this field is empty, no callback will be sent
    pub receive_callback_data: Option<Binary>,
    /// The address that will receive the callback message
    /// Defaults to the receiver address
    pub receive_callback_addr: Option<String>,
}

/// A message is that is being called on receiving the NFT after transfer was completed.
/// Receiving this message means that the NFT was successfully transferred.
/// You must verify this message was called by an approved ICS721 contract, either by code_id or address.
#[cw_serde]
pub struct Ics721ReceiveCallbackMsg {
    pub nft_contract: String,
    pub original_packet: NonFungibleTokenPacketData,
    pub msg: Binary,
}

/// A message to update your contract of the status of a transfer
/// status = Ics721Status::Success - the transfer was successful and NFT is on the other chain
/// status = Ics721Status::Failed - Transfer failed and contract still owns the NFT
#[cw_serde]
pub struct Ics721AckCallbackMsg {
    pub status: Ics721Status,
    pub nft_contract: String,
    pub original_packet: NonFungibleTokenPacketData,
    pub msg: Binary,
}

/// The status of a transfer on callback
#[cw_serde]
pub enum Ics721Status {
    Success,
    Failed(String),
}

/// This is a wrapper for ics721 callbacks
/// so contracts will be able to recieve both status update and on receive hook.
#[cw_serde]
pub enum ReceiverExecuteMsg {
    /// Being called on receiving the NFT after transfer was completed. (destination side)
    /// `on_recieve` hook
    /// Note - Failing this message will fail the transfer.
    Ics721ReceiveCallback(Ics721ReceiveCallbackMsg),
    /// Being called as a status update of the transfer. (source side)
    /// Note - Failing this message will NOT fail the transfer, its just a status update.
    Ics721AckCallback(Ics721AckCallbackMsg),

    /// Being called on receiving the NFT before transfer is completed. (destination side)
    /// `on_recieve` hook
    /// Note - Failing this message will fail the transfer.
    Ics721ReceivePacketMsg {
        packet: IbcPacket,
        data: NonFungibleTokenPacketData,
    },
}
