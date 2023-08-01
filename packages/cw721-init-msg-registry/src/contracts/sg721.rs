use cosmwasm_std::{to_binary, Binary, StdResult};

use crate::data::Ics721Data;

/// Generate sg721 init message given ics721 data
pub fn ics721_get_init_msg(data: &Ics721Data) -> StdResult<Binary> {
    to_binary(&sg721::InstantiateMsg {
        name: data.class_id.to_string(),
        symbol: data.class_id.to_string(),
        minter: data.ics721_addr.to_string(),
        collection_info: sg721::CollectionInfo {
            creator: data.ics721_addr.to_string(),
            description: "Ics721 created collection".to_string(),
            image: "".to_string(),
            external_link: None,
            explicit_content: None,
            start_trading_time: None,
            royalty_info: None,
        },
    })
}
