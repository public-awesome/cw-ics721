use cosmwasm_std::entry_point;
use cosmwasm_std::Response;
use cosmwasm_std::StdError;
#[cfg(not(feature = "library"))]
use cosmwasm_std::{to_binary, Binary, Deps, DepsMut, Empty, Env, MessageInfo, StdResult};
use cw721_base_ibc::msg::{ExecuteMsg, InstantiateMsg, MintMsg, QueryMsg};
use cw721_base_ibc::{ContractError, Cw721Contract};
use cw721_ibc::{Cw721Execute, Cw721Query, NftInfoResponse, OwnerOfResponse};

pub type CW721ContractWrapper<'a> = Cw721Contract<'a, Empty, Empty>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<cosmwasm_std::Response> {
    CW721ContractWrapper::default().instantiate(deps, _env, _info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<Empty>,
) -> Result<Response<Empty>, ContractError> {
    CW721ContractWrapper::default().execute(deps, env, info, msg)
}

pub fn transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    class_id: String,
    token_id: String,
) -> Result<cosmwasm_std::Response, ContractError> {
    CW721ContractWrapper::default().transfer_nft(deps, env, info, recipient, class_id, token_id)
}

pub fn mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
    token_uri: String,
    receiver: String,
) -> Result<cosmwasm_std::Response, ContractError> {
    let mint_msg = MintMsg {
        class_id,
        token_id,
        owner: receiver,
        token_uri: Some(token_uri),
        extension: Empty {},
    };
    CW721ContractWrapper::default().mint(deps, _env, info, mint_msg)
}

pub fn burn(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    class_id: String,
    token_id: String,
) -> Result<cosmwasm_std::Response, ContractError> {
    CW721ContractWrapper::default().burn(deps, _env, info, class_id, token_id)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::OwnerOf {
            class_id,
            token_id,
            include_expired,
        } => to_binary(&get_owner(
            deps,
            _env,
            class_id,
            token_id,
            include_expired.unwrap_or(false),
        )?),
        QueryMsg::NftInfo { class_id, token_id } => to_binary(&nft_info(deps, class_id, token_id)?),
        _ => Err(StdError::GenericErr {
            msg: "Unsupported message type".to_string(),
        }),
    }
}

pub fn get_owner(
    deps: Deps,
    env: Env,
    class_id: String,
    token_id: String,
    include_expired: bool,
) -> StdResult<OwnerOfResponse> {
    CW721ContractWrapper::default().owner_of(deps, env, class_id, token_id, include_expired)
}

fn nft_info(deps: Deps, class_id: String, token_id: String) -> StdResult<NftInfoResponse<Empty>> {
    CW721ContractWrapper::default().nft_info(deps, class_id, token_id)
}
