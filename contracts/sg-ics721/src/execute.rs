use cosmwasm_std::{from_binary, to_binary, Binary, Env, StdResult};
use ics721::{execute::Ics721Execute, state::ClassData, token_types::Class};

use crate::state::SgIcs721Contract;

impl Ics721Execute for SgIcs721Contract {
    fn init_msg(&self, env: &Env, class: &Class) -> StdResult<Binary> {
        let creator = match class.data.clone() {
            // in case no class data is provided, ics721 will be used as the creator
            None => env.contract.address.to_string(),
            Some(data) => {
                let class_data: ClassData = from_binary(&data)?;

                match class_data.owner {
                    // in case no owner is provided, ics721 will be used as the creator
                    None => env.contract.address.to_string(),
                    Some(source_owner) => {
                        // convert the source owner (e.g. `juno1XXX`) to target owner (e.g. `stars1XXX`)
                        let (_source_hrp, source_data, source_variant) =
                            bech32::decode(source_owner.as_str()).unwrap();
                        // detect target hrp (e.g. `stars`) using contract address
                        let (target_hrp, _targete_data, _target_variant) =
                            bech32::decode(env.contract.address.as_str()).unwrap();
                        // convert source owner to target owner
                        let target_owner =
                            bech32::encode(target_hrp.as_str(), source_data, source_variant)
                                .unwrap();
                        target_owner
                    }
                }
            }
        };
        to_binary(&sg721::InstantiateMsg {
            // Name of the collection MUST be class_id as this is how
            // we create a map entry on reply.
            name: class.id.clone().into(),
            symbol: class.id.clone().into(),
            minter: env.contract.address.to_string(),
            collection_info: sg721::CollectionInfo {
                creator,
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
