use cosmwasm_std::{Addr, DepsMut, Empty, Env, StdResult};
use cw721::msg::NumTokensResponse;
use cw_ownable::Ownership;

use crate::state::{CollectionData, UniversalCollectionInfoResponse};

pub fn get_collection_data(deps: &DepsMut, collection: &Addr) -> StdResult<CollectionData> {
    // cw721 v0.19 and higher holds creator ownership in the contract
    let ownership_result: StdResult<Ownership<Addr>> = deps.querier.query_wasm_smart(
        collection,
        &cw721_metadata_onchain::QueryMsg::GetCreatorOwnership {},
    );
    let owner = match ownership_result {
        Ok(ownership) => ownership.owner.map(|a| a.to_string()),
        Err(_) => {
            // cw721 v0.17 and v0.18 holds minter ownership in the contract
            let ownership: StdResult<Ownership<Addr>> = deps.querier.query_wasm_smart(
                collection,
                &cw721_metadata_onchain::QueryMsg::GetMinterOwnership {},
            );
            match ownership {
                Ok(ownership) => ownership.owner.map(|a| a.to_string()),
                Err(_) => {
                    // cw721 v0.16 and lower holds minter
                    println!(">>> cw721 v0.16 and lower holds minter");
                    let minter_response: cw721_base_016::msg::MinterResponse =
                        deps.querier.query_wasm_smart(
                            collection,
                            &cw721_base_016::QueryMsg::Minter::<Empty> {},
                        )?;
                    deps.api.addr_validate(&minter_response.minter)?;
                    Some(minter_response.minter)
                }
            }
        }
    };
    println!(">>> owner: {:?}", owner);
    let contract_info = deps.querier.query_wasm_contract_info(collection)?;
    println!(">>> contract_info: {:?}", contract_info);
    let UniversalCollectionInfoResponse { name, symbol } = deps.querier.query_wasm_smart(
        collection,
        #[allow(deprecated)]
        // For now we use `ContractInfo` which is known across all version, whilst `GetCollectionInfoAndExtension` is only available in v0.19 and higher
        &cw721_metadata_onchain::QueryMsg::ContractInfo {},
    )?;
    let NumTokensResponse { count } = deps
        .querier
        .query_wasm_smart(collection, &cw721_metadata_onchain::QueryMsg::NumTokens {})?;

    Ok(CollectionData {
        owner,
        contract_info: Some(contract_info),
        name,
        symbol,
        num_tokens: Some(count),
    })
}

/// Convert owner chain address (e.g. `juno1XXX`) to target owner chain address (e.g. `stars1XXX`).
pub fn convert_owner_chain_address(env: &Env, source_owner: &str) -> StdResult<String> {
    // convert the source owner (e.g. `juno1XXX`) to target owner (e.g. `stars1XXX`)
    let (_source_hrp, source_data, source_variant) = bech32::decode(source_owner).unwrap();
    // detect target hrp (e.g. `stars`) using contract address
    let (target_hrp, _target_data, _target_variant) =
        bech32::decode(env.contract.address.as_str()).unwrap();
    // convert source owner to target owner
    let target_owner = bech32::encode(target_hrp.as_str(), source_data, source_variant).unwrap();
    Ok(target_owner)
}
