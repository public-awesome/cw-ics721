use cosmwasm_std::{to_binary, Binary, StdResult};

use crate::data::InitMsgData;

/// Generate cw721-base init message given ics721 data
pub fn ics721_get_init_msg(data: &InitMsgData) -> StdResult<Binary> {
    to_binary(&cw721_base::InstantiateMsg {
        name: data.class_id.to_string(),
        symbol: data.class_id.to_string(),
        minter: data.ics721_addr.to_string(),
    })
}
