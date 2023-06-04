use cosmwasm_schema::cw_serde;
use cosmwasm_std::IbcTimeout;

/// A struct to handle callbacks of ICS721 transfers.
///
/// If you have a contract that does sending and receiving
/// you can have a simple structure of callbacks like this:
/// ```rust
/// pub enum Ics721Callbacks {
///    NftSent {},
///    NftReceived {},
/// }
/// ```
/// `NftSent` is called after nft was transfered on the `sender`
///
/// `NftReceived` is called after nft was transfered on the `receiver`
#[cw_serde]
pub enum Ics721Callbacks {
    /// We notify the sender that the NFT was sent successfuly.
    NftSent {},
    /// NFT was sent successfuly, but we fail the callback for tests.
    NftSentButFailCallback {},
    /// Do something on the receiving chain once the NFT was sent.
    NftReceived {},
    /// NFT was sent successfuly, but we fail the callback for tests.
    NftReceivedButFailCallback {},
}

#[cw_serde]
pub enum AckMode {
    // Messages should respond with an error ACK.
    Error,
    // Messages should respond with a success ACK.
    Success,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub ack_mode: AckMode,
}

#[cw_serde]
#[allow(clippy::large_enum_variant)] // `data` field is a bit large
                                     // for clippy's taste.
pub enum ExecuteMsg {
    Ics721ReceiveMsg(ics721::Ics721ReceiveMsg),
    CloseChannel {
        channel_id: String,
    },
    SendPacket {
        channel_id: String,
        timeout: IbcTimeout,

        data: ics721::NonFungibleTokenPacketData,
    },
    SetAckMode {
        ack_mode: AckMode,
    },
}

#[cw_serde]
pub enum QueryMsg {
    /// Gets the current ack mode. Returns `AckMode`.
    AckMode {},
    /// Gets the mode of the last ack this contract received. Errors
    /// if no ACK has ever been received. Returns `AckMode`.
    LastAck {},
}
