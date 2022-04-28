#[cfg(not(feature = "library"))]
use cosmwasm_std::entry_point;
use cosmwasm_std::{DepsMut, Empty, Env, MessageInfo, StdResult};
use cw721_base;
use cw721_base::msg::InstantiateMsg;

pub type CW721ContractWrapper<'a> = cw721_base::Cw721Contract<'a, Empty, Empty>;

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<cosmwasm_std::Response> {
    CW721ContractWrapper::default().instantiate(deps, _env, _info, msg)
}
