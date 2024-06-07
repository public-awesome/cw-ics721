use cosmwasm_std::{from_json, to_json_binary, Addr, Binary, Deps, DepsMut, Env, StdResult};
use ics721::{execute::Ics721Execute, state::CollectionData, utils::get_collection_data};
use ics721_types::token_types::Class;

use sg721_base::msg::{CollectionInfoResponse, QueryMsg};

use crate::state::{SgCollectionData, SgIcs721Contract, STARGAZE_ICON_PLACEHOLDER};

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
            collection_info: Some(collection_info),
        }))
    }

    fn init_msg(
        &self,
        deps: Deps,
        env: &Env,
        class: &Class,
        _cw721_admin: Option<String>,
    ) -> StdResult<Binary> {
        // ics721 creator is used, in case no source owner in class data is provided (e.g. due to nft-transfer module).
        let ics721_contract_info = deps
            .querier
            .query_wasm_contract_info(env.contract.address.to_string())?;
        // use by default ClassId, in case there's no class data with name and symbol
        let mut instantiate_msg = sg721::InstantiateMsg {
            name: class.id.clone().into(),
            symbol: class.id.clone().into(),
            minter: env.contract.address.to_string(),
            // creator: cw721_admin, // TODO: once sg721 migrates to cw721 v19, use cw721_admin for setting creator
            collection_info: sg721::CollectionInfo {
                // source owner could be: 1. regular wallet, 2. contract, or 3. multisig
                // bech32 calculation for 2. and 3. leads to unknown address
                // therefore, we use ics721 creator as owner
                creator: ics721_contract_info.creator,
                description: "".to_string(),
                // use Stargaze icon as placeholder
                image: STARGAZE_ICON_PLACEHOLDER.to_string(),
                external_link: None,
                explicit_content: None,
                start_trading_time: None,
                royalty_info: None,
            },
        };

        // use collection data for setting name and symbol
        let collection_data = class
            .data
            .clone()
            .and_then(|binary| from_json::<CollectionData>(binary).ok());
        if let Some(collection_data) = collection_data {
            instantiate_msg.name = collection_data.name;
            instantiate_msg.symbol = collection_data.symbol;
        }

        to_json_binary(&instantiate_msg)
    }
}
