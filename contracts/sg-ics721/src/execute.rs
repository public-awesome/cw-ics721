use cosmwasm_std::{
    from_json, to_json_binary, Addr, Binary, ContractInfoResponse, Deps, DepsMut, Env, StdResult,
};
use cw721::{CollectionExtension, NftExtension, RoyaltyInfo};
use ics721::{execute::Ics721Execute, state::CollectionData, utils::get_collection_data};
use ics721_types::token_types::Class;

use sg721::RoyaltyInfoResponse;
use sg721_base::msg::CollectionInfoResponse;
use sg721_metadata_onchain::QueryMsg;
use sg_metadata::{Metadata, Trait};

use crate::state::{SgIcs721Contract, STARGAZE_ICON_PLACEHOLDER};

impl Ics721Execute for SgIcs721Contract {
    type ClassData = CollectionData;

    /// sg-ics721 sends custom SgCollectionData, basically it extends ics721-base::state::CollectionData with additional collection_info.
    fn get_class_data(&self, deps: &DepsMut, sender: &Addr) -> StdResult<Option<Self::ClassData>> {
        let CollectionData {
            owner,
            contract_info,
            name,
            symbol,
            extension: _, // ignore extension coming from standard cw721, since sg721 has its own extension (collection info)
            num_tokens,
        } = get_collection_data(deps, sender)?;
        let collection_info: CollectionInfoResponse = deps
            .querier
            .query_wasm_smart(sender, &QueryMsg::CollectionInfo {})?;
        let royalty_info = collection_info.royalty_info.map(|r| RoyaltyInfo {
            payment_address: Addr::unchecked(r.payment_address),
            share: r.share,
        });
        let extension = Some(CollectionExtension {
            description: collection_info.description,
            image: collection_info.image,
            external_link: collection_info.external_link,
            explicit_content: collection_info.explicit_content,
            start_trading_time: collection_info.start_trading_time,
            royalty_info,
        });

        Ok(Some(CollectionData {
            owner,
            contract_info,
            name,
            symbol,
            num_tokens,
            extension,
        }))
    }

    fn init_msg(
        &self,
        deps: Deps,
        env: &Env,
        class: &Class,
        cw721_admin: Option<String>,
    ) -> StdResult<Binary> {
        // ics721 creator is used, in case no source owner in class data is provided (e.g. due to nft-transfer module).
        let ContractInfoResponse { creator, admin, .. } = deps
            .querier
            .query_wasm_contract_info(env.contract.address.to_string())?;
        // use by default ClassId, in case there's no class data with name and symbol
        let cw721_admin_or_ics721_admin_or_ics721_creator = cw721_admin
            .clone()
            .or_else(|| admin.clone())
            .or_else(|| Some(creator.clone()))
            .unwrap();
        let mut instantiate_msg = sg721::InstantiateMsg {
            name: class.id.clone().into(),
            symbol: class.id.clone().into(),
            minter: env.contract.address.to_string(),
            collection_info: sg721::CollectionInfo {
                // source owner could be: 1. regular wallet, 2. contract, or 3. multisig
                // bech32 calculation for 2. and 3. leads to unknown address
                // therefore, we use ics721 creator as owner
                creator: cw721_admin_or_ics721_admin_or_ics721_creator.clone(),
                description: "".to_string(),
                // remaining props is set below, in case there's collection data
                image: STARGAZE_ICON_PLACEHOLDER.to_string(), // use Stargaze icon as placeholder
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
            if let Some(collection_info_extension_msg) =
                collection_data.extension.map(|ext| sg721::CollectionInfo {
                    creator: cw721_admin_or_ics721_admin_or_ics721_creator.clone(),
                    description: ext.description,
                    image: ext.image,
                    external_link: ext.external_link,
                    explicit_content: ext.explicit_content,
                    start_trading_time: ext.start_trading_time,
                    royalty_info: ext.royalty_info.map(|r| RoyaltyInfoResponse {
                        payment_address: cw721_admin_or_ics721_admin_or_ics721_creator, // r.payment_address cant be used, since it is from another chain
                        share: r.share,
                    }),
                })
            {
                instantiate_msg.collection_info = collection_info_extension_msg;
            }
        }

        to_json_binary(&instantiate_msg)
    }

    fn mint_msg(
        &self,
        token_id: String,
        token_uri: Option<String>,
        owner: String,
        data: Option<Binary>,
    ) -> StdResult<Binary> {
        // parse token data and check whether it is of type NftExtension
        let extension = data
            .and_then(|binary| {
                from_json::<NftExtension>(binary).ok().map(|ext| Metadata {
                    animation_url: ext.animation_url,
                    attributes: ext.attributes.map(|traits| {
                        traits
                            .into_iter()
                            .map(|t| Trait {
                                trait_type: t.trait_type,
                                value: t.value,
                                display_type: t.display_type,
                            })
                            .collect()
                    }),
                    background_color: ext.background_color,
                    description: ext.description,
                    external_url: ext.external_url,
                    image: ext.image,
                    image_data: ext.image_data,
                    youtube_url: ext.youtube_url,
                    name: ext.name,
                })
            })
            .unwrap_or(Metadata {
                // no onchain metadata (only offchain), in this case empty metadata is created
                ..Default::default()
            });

        let msg = sg721_metadata_onchain::ExecuteMsg::Mint {
            token_id,
            token_uri, // holds off-chain metadata
            owner,
            extension, // holds on-chain metadata
        };
        to_json_binary(&msg)
    }
}
