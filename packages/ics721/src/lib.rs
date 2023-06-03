use cosmwasm_schema::cw_serde;
use cosmwasm_std::Binary;

#[cw_serde]
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
