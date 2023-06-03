use cosmwasm_schema::{
    cw_serde,
    schemars::JsonSchema,
    serde::{Deserialize, Serialize}
};
use cosmwasm_std::Binary;

// cw_serde includes: `deny_unknown_fields`
// This means that it cw_serde expects the exect struct to be parsed
// but in this specific case we only want to parse what ics721 accepts
// and ignore everything else

// This allows anyone to pass any memo they like
// and ics721 will only pick the things it knows how to handle
// the very basic example is callbacks.
#[derive(Serialize, Deserialize, JsonSchema, Clone, Debug, PartialEq)]
#[allow(clippy::derive_partial_eq_without_eq)]
#[schemars(crate = "cosmwasm_schema::schemars")]
#[serde(crate = "cosmwasm_schema::serde")]
pub struct Ics721Memo {
    pub callbacks: Option<Ics721Callbacks>,
}

#[cw_serde]
pub struct Ics721Callbacks {
    pub src_callback_msg: Option<Binary>,
    pub dest_callback_msg: Option<Binary>,
}

#[cw_serde]
pub struct Ics721ReceiveMsg {
    pub status: Ics721Status,
    pub msg: Binary,
}

#[cw_serde]
pub enum Ics721Status {
    Success,
    Failed,
}
