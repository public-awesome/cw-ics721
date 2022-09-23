use cosmwasm_std::IbcTimeout;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AckMode {
    // Messages should respond with an error ACK.
    Error,
    // Messages should respond with a success ACK.
    Success,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct InstantiateMsg {
    pub ack_mode: AckMode,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    CloseChannel {
        channel_id: String,
    },
    SendPacket {
        channel_id: String,
        timeout: IbcTimeout,

        data: cw_ics721_bridge::ibc::NonFungibleTokenPacketData,
    },
    SetAckMode {
        ack_mode: AckMode,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Gets the current ack mode. Returns `AckMode`.
    AckMode {},
    /// Gets the mode of the last ack this contract received. Errors
    /// if no ACK has ever been received. Returns `AckMode`.
    LastAck {},
}
