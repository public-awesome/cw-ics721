use cosmwasm_std::{Addr, DepsMut, Empty, Env, StdResult};
use cw721::msg::NumTokensResponse;
use cw_ownable::Ownership;

use crate::state::{CollectionData, UniversalCollectionInfoResponse};

pub fn get_collection_data(deps: &DepsMut, collection: &Addr) -> StdResult<CollectionData> {
    // cw721 v0.19 and higher holds creator ownership (cw-ownable storage) in the contract
    let ownership_result: StdResult<Ownership<Addr>> = deps.querier.query_wasm_smart(
        collection,
        &cw721_metadata_onchain::msg::QueryMsg::GetCreatorOwnership {},
    );
    let owner = match ownership_result {
        Ok(ownership) => ownership.owner.map(|a| a.to_string()),
        Err(_) => {
            // cw721 v0.17 and v0.18 holds minter ownership (cw-ownable storage) in the contract
            let ownership: StdResult<Ownership<Addr>> = deps.querier.query_wasm_smart(
                collection,
                &cw721_base_018::msg::QueryMsg::Ownership::<Addr> {}, // nb: could also use `GetMinterOwnership`, but some custom contracts may only know about `Ownership`
            );
            match ownership {
                Ok(ownership) => ownership.owner.map(|a| a.to_string()),
                Err(_) => {
                    // cw721 v0.16 and lower holds minter (simple string storage)
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
    let contract_info = deps.querier.query_wasm_contract_info(collection)?;
    let UniversalCollectionInfoResponse {
        name,
        symbol,
        extension,
        updated_at: _,
    } = deps.querier.query_wasm_smart(
        collection,
        #[allow(deprecated)]
        // For now we use `ContractInfo` which is known across all version, whilst `GetCollectionInfoAndExtension` is only available in v0.19 and higher
        &cw721_metadata_onchain::msg::QueryMsg::ContractInfo {},
    )?;
    let NumTokensResponse { count } = deps.querier.query_wasm_smart(
        collection,
        &cw721_metadata_onchain::msg::QueryMsg::NumTokens {},
    )?;

    Ok(CollectionData {
        owner,
        contract_info: Some(contract_info),
        num_tokens: Some(count),
        name,
        symbol,
        extension,
    })
}

/// Convert owner chain address (e.g. `juno1XXX`) to target owner chain address (e.g. `stars1XXX`).
pub fn convert_owner_chain_address(env: &Env, source_owner: &str) -> StdResult<String> {
    // convert the source owner (e.g. `juno1XXX`) to target owner (e.g. `stars1XXX`)
    let (_source_hrp, source_data) = bech32::decode(source_owner).unwrap();
    // detect target hrp (e.g. `stars`) using contract address
    let (target_hrp, _target_data) = bech32::decode(env.contract.address.as_str()).unwrap();
    // convert source owner to target owner
    let target_owner = bech32::encode::<bech32::Bech32>(target_hrp, &source_data).unwrap();
    Ok(target_owner)
}
