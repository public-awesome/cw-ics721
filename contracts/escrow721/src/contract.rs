#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Deps, Empty, Env, MessageInfo, StdResult, to_binary, Binary, StdError};
use cw721::{Cw721Execute, OwnerOfResponse};
use cw721_base::Cw721Contract;
use cw721_base::msg::{InstantiateMsg, MintMsg, ExecuteMsg, QueryMsg};
use cw721_base::ContractError;
use cw721_base::helpers::Cw721Contract as Cw721ContractHelper;
use crate::msg;


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

pub fn transfer(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    recipient: String,
    token_id: String,
) -> Result<cosmwasm_std::Response, ContractError> {

    CW721ContractWrapper::default().transfer_nft(deps, env, info, recipient, token_id)

}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg<Empty>,
) -> Result<cosmwasm_std::Response, ContractError> {
    println!("in the execute");
    match msg {
       ExecuteMsg::Mint(msg ) => mint(deps, env, info, msg),
        _ => Err(ContractError::Expired {  })
    }
}

pub fn mint(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: MintMsg<Empty>,
) -> Result<cosmwasm_std::Response, ContractError> {

    CW721ContractWrapper::default().mint(deps, _env, info, msg)
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(
    deps: Deps, 
    _env: Env, 
    msg: QueryMsg
    ) -> StdResult<Binary> {
    println!("in the query");
    match msg {
        // QueryMsg::OwnerOf{token_id,include_expired}=>to_binary(
        //     get_owner(deps, _env, token_id, true)
        // ),
         _ => CW721ContractWrapper::default().query(deps, _env, msg.into()),
}}

pub fn get_owner(
    deps: Deps,
    env: Env,
    token_id: String,
    include_expired: bool,
) -> StdResult<OwnerOfResponse> {
    print!("in the get owner");
    Cw721ContractHelper(env.contract.address).owner_of(&deps.querier, token_id, include_expired)
}