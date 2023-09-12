use cosmwasm_std::{from_binary, to_binary, Addr, Binary, Deps, DepsMut, Env, StdResult};
use ics721::{
    execute::Ics721Execute,
    state::CollectionData,
    token_types::Class,
    utils::{convert_owner_chain_address, get_collection_data},
};
use sg721_base::msg::{CollectionInfoResponse, QueryMsg};

use crate::state::{SgCollectionData, SgIcs721Contract};

impl Ics721Execute for SgIcs721Contract {
    type ClassData = SgCollectionData;

    /// sg-ics721 sends custom SgCollectionData, basically it extends ics721-base::state::CollectionData with additional collection_info.
    fn get_class_data(&self, deps: &DepsMut, sender: &Addr) -> StdResult<Option<Self::ClassData>> {
        let CollectionData {
            owner,
            contract_info,
            name,
            symbol,
            num_tokens,
        } = get_collection_data(deps, sender)?;
        let collection_info: CollectionInfoResponse = deps
            .querier
            .query_wasm_smart(sender, &QueryMsg::CollectionInfo {})?;

        Ok(Some(SgCollectionData {
            owner,
            contract_info,
            name,
            symbol,
            num_tokens,
            collection_info,
        }))
    }

    fn init_msg(&self, deps: Deps, env: &Env, class: &Class) -> StdResult<Binary> {
        let creator = match class.data.clone() {
            None => {
                // in case no class data is provided (e.g. due to nft-transfer module), ics721 creator is used.
                let contract_info = deps
                    .querier
                    .query_wasm_contract_info(env.contract.address.to_string())?;
                contract_info.creator
            }
            Some(data) => {
                // class data may be any custom type. Check whether it is `ics721::state::CollectionData` or not.
                let class_data_result: StdResult<CollectionData> = from_binary(&data);
                if class_data_result.is_err() {
                    // this happens only for unknown class data, like source chain uses nft-transfer module
                    env.contract.address.to_string()
                } else {
                    let class_data = class_data_result?;

                    match class_data.owner {
                        Some(owner) => convert_owner_chain_address(env, owner.as_str())?,
                        None => env.contract.address.to_string(),
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
