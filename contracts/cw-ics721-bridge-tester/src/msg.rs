use cosmwasm_schema::cw_serde;
use cosmwasm_std::IbcTimeout;

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
    CloseChannel {
        channel_id: String,
    },
    SendPacket {
        channel_id: String,
        timeout: IbcTimeout,

        data: ics721::ibc::NonFungibleTokenPacketData,
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
