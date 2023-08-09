use cosmwasm_std::{to_binary, Binary, Env, StdResult};
use ics721::{execute::Ics721Execute, token_types::Class};
use sg_std::StargazeMsgWrapper;

use crate::state::SgIcs721Contract;

impl Ics721Execute<StargazeMsgWrapper> for SgIcs721Contract {
    fn init_msg(&self, env: &Env, class: &Class) -> StdResult<Binary> {
        to_binary(&sg721::InstantiateMsg {
            // Name of the collection MUST be class_id as this is how
            // we create a map entry on reply.
            name: class.id.clone().into(),
            symbol: class.id.clone().into(),
            minter: env.contract.address.to_string(),
            collection_info: sg721::CollectionInfo {
                creator: env.contract.address.to_string(),
                description: "".to_string(),
                image: "https://arkprotocol.io".to_string(),
                external_link: None,
                explicit_content: None,
                start_trading_time: None,
                royalty_info: None,
            },
        })
    }
}
