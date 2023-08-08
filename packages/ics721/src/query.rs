use cosmwasm_std::{to_binary, Addr, Binary, Deps, Env, Order, StdResult};
use cw_storage_plus::Map;

use crate::{
    msg::{ClassToken, QueryMsg},
    state::{Ics721Contract, UniversalAllNftInfoResponse},
    token_types::{Class, ClassId, Token, TokenId},
};

pub trait Ics721Query {
    fn query(&self, deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
        match msg {
            QueryMsg::ClassId { contract } => {
                to_binary(&self.query_class_id_for_nft_contract(deps, contract)?)
            }
            QueryMsg::NftContract { class_id } => {
                to_binary(&self.query_nft_contract_for_class_id(deps, class_id)?)
            }
            QueryMsg::ClassMetadata { class_id } => {
                to_binary(&self.query_class_metadata(deps, class_id)?)
            }
            QueryMsg::TokenMetadata { class_id, token_id } => {
                to_binary(&self.query_token_metadata(deps, class_id, token_id)?)
            }
            QueryMsg::Owner { class_id, token_id } => {
                to_binary(&self.query_owner(deps, class_id, token_id)?)
            }
            QueryMsg::Pauser {} => {
                to_binary(&Ics721Contract::default().po.query_pauser(deps.storage)?)
            }
            QueryMsg::Paused {} => {
                to_binary(&Ics721Contract::default().po.query_paused(deps.storage)?)
            }
            QueryMsg::Proxy {} => to_binary(&Ics721Contract::default().proxy.load(deps.storage)?),
            QueryMsg::Cw721CodeId {} => to_binary(&self.query_cw721_code_id(deps)?),
            QueryMsg::NftContracts { start_after, limit } => {
                to_binary(&self.query_nft_contracts(deps, start_after, limit)?)
            }
            QueryMsg::OutgoingChannels { start_after, limit } => to_binary(&query_channels(
                deps,
                &Ics721Contract::default()
                    .channels_info
                    .outgoing_class_token_to_channel,
                start_after,
                limit,
            )?),
            QueryMsg::IncomingChannels { start_after, limit } => to_binary(&query_channels(
                deps,
                &Ics721Contract::default()
                    .channels_info
                    .incoming_class_token_to_channel,
                start_after,
                limit,
            )?),
        }
    }

    fn query_class_id_for_nft_contract(
        &self,
        deps: Deps,
        contract: String,
    ) -> StdResult<Option<ClassId>> {
        let contract = deps.api.addr_validate(&contract)?;
        Ics721Contract::default()
            .class_id_info
            .nft_contract_to_class_id
            .may_load(deps.storage, contract)
    }

    fn query_nft_contract_for_class_id(
        &self,
        deps: Deps,
        class_id: String,
    ) -> StdResult<Option<Addr>> {
        Ics721Contract::default()
            .class_id_info
            .class_id_to_nft_contract
            .may_load(deps.storage, ClassId::new(class_id))
    }

    fn query_class_metadata(&self, deps: Deps, class_id: String) -> StdResult<Option<Class>> {
        Ics721Contract::default()
            .class_id_info
            .class_id_to_class
            .may_load(deps.storage, ClassId::new(class_id))
    }

    fn query_token_metadata(
        &self,
        deps: Deps,
        class_id: String,
        token_id: String,
    ) -> StdResult<Option<Token>> {
        let token_id = TokenId::new(token_id);
        let class_id = ClassId::new(class_id);

        let Some(token_metadata) = Ics721Contract::default().cw721_info.token_metadata.may_load(
            deps.storage,
            (class_id.clone(), token_id.clone()),
        )? else {
        // Token metadata is set unconditionaly on mint. If we have no
        // metadata entry, we have no entry for this token at all.
        return Ok(None)
        };
        let Some(token_contract) = Ics721Contract::default().class_id_info.class_id_to_nft_contract.may_load(
        deps.storage,
        class_id
        )? else {
        debug_assert!(false, "token_metadata != None => token_contract != None");
        return Ok(None)
        };
        let UniversalAllNftInfoResponse { info, .. } = deps.querier.query_wasm_smart(
            token_contract,
            &cw721::Cw721QueryMsg::AllNftInfo {
                token_id: token_id.clone().into(),
                include_expired: None,
            },
        )?;
        Ok(Some(Token {
            id: token_id,
            uri: info.token_uri,
            data: token_metadata,
        }))
    }

    fn query_owner(
        &self,
        deps: Deps,
        class_id: String,
        token_id: String,
    ) -> StdResult<cw721::OwnerOfResponse> {
        let class_uri = Ics721Contract::default()
            .class_id_info
            .class_id_to_nft_contract
            .load(deps.storage, ClassId::new(class_id))?;
        let resp: cw721::OwnerOfResponse = deps.querier.query_wasm_smart(
            class_uri,
            &cw721::Cw721QueryMsg::OwnerOf {
                token_id,
                include_expired: None,
            },
        )?;
        Ok(resp)
    }

    fn query_cw721_code_id(&self, deps: Deps) -> StdResult<u64> {
        Ics721Contract::default()
            .cw721_info
            .cw721_code_id
            .load(deps.storage)
    }

    fn query_nft_contracts(
        &self,
        deps: Deps,
        start_after: Option<ClassId>,
        limit: Option<u32>,
    ) -> StdResult<Vec<(String, Addr)>> {
        cw_paginate_storage::paginate_map(
            deps,
            &Ics721Contract::default()
                .class_id_info
                .class_id_to_nft_contract,
            start_after,
            limit,
            Order::Ascending,
        )
    }
}

fn query_channels(
    deps: Deps,
    class_token_to_channel: &Map<(ClassId, TokenId), String>,
    start_after: Option<ClassToken>,
    limit: Option<u32>,
) -> StdResult<Vec<((String, String), String)>> {
    let start_after = start_after.map(|class_token| {
        (
            ClassId::new(class_token.class_id),
            TokenId::new(class_token.token_id),
        )
    });
    cw_paginate_storage::paginate_map(
        deps,
        class_token_to_channel,
        start_after,
        limit,
        Order::Ascending,
    )
}

impl Ics721Query for Ics721Contract<'static> {}
