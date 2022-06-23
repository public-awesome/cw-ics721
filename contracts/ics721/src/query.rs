use crate::msg::{ChannelResponse, QueryMsg};
use crate::state::{CHANNEL_INFO, CHANNEL_STATE};

#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{to_binary, Binary, Deps, Env, IbcQuery, Order, PortIdResponse, StdResult};
use cw20_ics20::msg::{ListChannelsResponse, PortResponse};

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Port {} => to_binary(&query_port(deps)?),
        QueryMsg::ListChannels {} => to_binary(&query_list(deps)?),
        QueryMsg::Channel { id } => to_binary(&query_channel(deps, id)?),
        QueryMsg::Tokens {
            channel_id,
            class_id,
        } => to_binary(&query_tokens(deps, channel_id, class_id)?),
    }
}

fn query_port(deps: Deps) -> StdResult<PortResponse> {
    let query = IbcQuery::PortId {}.into();
    let PortIdResponse { port_id } = deps.querier.query(&query)?;
    Ok(PortResponse { port_id })
}

fn query_list(deps: Deps) -> StdResult<ListChannelsResponse> {
    let channels: StdResult<Vec<_>> = CHANNEL_INFO
        .range(deps.storage, None, None, Order::Ascending)
        .map(|r| r.map(|(_, v)| v))
        .collect();
    Ok(ListChannelsResponse {
        channels: channels?,
    })
}

pub fn query_channel(deps: Deps, id: String) -> StdResult<ChannelResponse> {
    let info = CHANNEL_INFO.load(deps.storage, &id)?;
    let _class_ids: StdResult<Vec<_>> = CHANNEL_STATE
        .sub_prefix(&id)
        .range(deps.storage, None, None, Order::Ascending)
        .map(|r| {
            let (class_id_token_id, _) = r?;
            Ok(class_id_token_id.0)
        })
        .collect();

    let class_ids_resp = _class_ids;
    match class_ids_resp {
        Ok(mut class_id_vec) => Ok(ChannelResponse {
            info,
            class_ids: {
                class_id_vec.sort();
                class_id_vec.dedup();
                class_id_vec
            },
        }),
        Err(msg) => Err(msg),
    }
}

// TODO: https://github.com/public-awesome/contracts/issues/59
pub fn query_tokens(
    _deps: Deps,
    _channel_id: String,
    _class_id: String,
) -> StdResult<ChannelResponse> {
    todo!()
}
