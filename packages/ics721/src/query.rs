use cosmwasm_std::{to_binary, Addr, Binary, Deps, Env, Order, StdResult};
use cw_storage_plus::Map;

use crate::{
    msg::{ClassToken, QueryMsg},
    state::{
        UniversalAllNftInfoResponse, CLASS_ID_TO_CLASS, CLASS_ID_TO_NFT_CONTRACT, CW721_CODE_ID,
        INCOMING_CLASS_TOKEN_TO_CHANNEL, NFT_CONTRACT_TO_CLASS_ID, OUTGOING_CLASS_TOKEN_TO_CHANNEL,
        PO, PROXY, TOKEN_METADATA,
    },
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
            QueryMsg::Pauser {} => to_binary(&PO.query_pauser(deps.storage)?),
            QueryMsg::Paused {} => to_binary(&PO.query_paused(deps.storage)?),
            QueryMsg::Proxy {} => to_binary(&PROXY.load(deps.storage)?),
            QueryMsg::Cw721CodeId {} => to_binary(&self.query_cw721_code_id(deps)?),
            QueryMsg::NftContracts { start_after, limit } => {
                to_binary(&self.query_nft_contracts(deps, start_after, limit)?)
            }
            QueryMsg::OutgoingChannels { start_after, limit } => to_binary(&query_channels(
                deps,
                &OUTGOING_CLASS_TOKEN_TO_CHANNEL,
                start_after,
                limit,
            )?),
            QueryMsg::IncomingChannels { start_after, limit } => to_binary(&query_channels(
                deps,
                &INCOMING_CLASS_TOKEN_TO_CHANNEL,
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
        NFT_CONTRACT_TO_CLASS_ID.may_load(deps.storage, contract)
    }

    fn query_nft_contract_for_class_id(
        &self,
        deps: Deps,
        class_id: String,
    ) -> StdResult<Option<Addr>> {
        CLASS_ID_TO_NFT_CONTRACT.may_load(deps.storage, ClassId::new(class_id))
    }

    fn query_class_metadata(&self, deps: Deps, class_id: String) -> StdResult<Option<Class>> {
        CLASS_ID_TO_CLASS.may_load(deps.storage, ClassId::new(class_id))
    }

    fn query_token_metadata(
        &self,
        deps: Deps,
        class_id: String,
        token_id: String,
    ) -> StdResult<Option<Token>> {
        let token_id = TokenId::new(token_id);
        let class_id = ClassId::new(class_id);

        let Some(token_metadata) = TOKEN_METADATA.may_load(
            deps.storage,
            (class_id.clone(), token_id.clone()),
        )? else {
        // Token metadata is set unconditionaly on mint. If we have no
        // metadata entry, we have no entry for this token at all.
        return Ok(None)
        };
        let Some(token_contract) = CLASS_ID_TO_NFT_CONTRACT.may_load(
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
        let class_uri = CLASS_ID_TO_NFT_CONTRACT.load(deps.storage, ClassId::new(class_id))?;
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
        CW721_CODE_ID.load(deps.storage)
    }

    fn query_nft_contracts(
        &self,
        deps: Deps,
        start_after: Option<ClassId>,
        limit: Option<u32>,
    ) -> StdResult<Vec<(String, Addr)>> {
        cw_paginate_storage::paginate_map(
            deps,
            &CLASS_ID_TO_NFT_CONTRACT,
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
