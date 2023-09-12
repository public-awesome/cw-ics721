use cosmwasm_std::{Addr, DepsMut, Empty, Env, StdResult};
use cw721::{ContractInfoResponse, NumTokensResponse};
use cw_ownable::Ownership;

use crate::state::CollectionData;

pub fn get_collection_data(deps: &DepsMut, collection: &Addr) -> StdResult<CollectionData> {
    // cw721 v0.17 and higher holds ownership in the contract
    let ownership: StdResult<Ownership<Addr>> = deps
        .querier
        .query_wasm_smart(collection, &cw721_base::msg::QueryMsg::Ownership::<Addr> {});
    let owner = match ownership {
        Ok(ownership) => ownership.owner.map(|a| a.to_string()),
        Err(_) => {
            // cw721 v0.16 and lower holds minter
            let minter_response: cw721_base_016::msg::MinterResponse = deps
                .querier
                .query_wasm_smart(collection, &cw721_base_016::QueryMsg::Minter::<Empty> {})?;
            deps.api.addr_validate(&minter_response.minter)?;
            Some(minter_response.minter)
        }
    };
    let contract_info = deps.querier.query_wasm_contract_info(collection)?;
    let ContractInfoResponse { name, symbol } = deps.querier.query_wasm_smart(
        collection,
        &cw721_base::msg::QueryMsg::<Empty>::ContractInfo {},
    )?;
    let NumTokensResponse { count } = deps.querier.query_wasm_smart(
        collection,
        &cw721_base::msg::QueryMsg::<Empty>::NumTokens {},
    )?;

    Ok(CollectionData {
        owner,
        contract_info,
        name,
        symbol,
        num_tokens: count,
    })
}

/// Convert owner chain address (e.g. `juno1XXX`) to target owner chain address (e.g. `stars1XXX`).
pub fn convert_owner_chain_address(env: &Env, source_owner: &str) -> StdResult<String> {
    // convert the source owner (e.g. `juno1XXX`) to target owner (e.g. `stars1XXX`)
    let (_source_hrp, source_data, source_variant) = bech32::decode(source_owner).unwrap();
    // detect target hrp (e.g. `stars`) using contract address
    let (target_hrp, _targete_data, _target_variant) =
        bech32::decode(env.contract.address.as_str()).unwrap();
    // convert source owner to target owner
    let target_owner = bech32::encode(target_hrp.as_str(), source_data, source_variant).unwrap();
    Ok(target_owner)
}
