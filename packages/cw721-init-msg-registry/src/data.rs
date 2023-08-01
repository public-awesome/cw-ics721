use cosmwasm_schema::cw_serde;

/// Struct to passdata from ics721 base contracts to the init msg function
#[cfg(feature = "ics721-base")]
#[cw_serde]
pub struct InitMsgData {
    pub class_id: String,
    pub ics721_addr: String,
}
