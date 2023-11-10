use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, WasmMsg};

#[cw_serde]
pub enum Admin {
    Address { addr: String },
    Instantiator {},
}

#[cw_serde]
pub struct ContractInstantiateInfo {
    pub code_id: u64,
    pub msg: Binary,
    pub admin: Option<Admin>,
    pub label: String,
}

impl ContractInstantiateInfo {
    pub fn into_wasm_msg(self, instantiator: Addr) -> WasmMsg {
        WasmMsg::Instantiate {
            admin: self.admin.map(|admin| match admin {
                Admin::Address { addr } => addr,
                Admin::Instantiator {} => instantiator.into_string(),
            }),
            code_id: self.code_id,
            msg: self.msg,
            label: self.label,
            funds: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use cosmwasm_std::{to_json_binary, Addr, WasmMsg};

    use super::*;

    #[test]
    fn test_instantiate_admin_none() {
        let no_admin = ContractInstantiateInfo {
            code_id: 42,
            msg: to_json_binary("foo").unwrap(),
            admin: None,
            label: "bar".to_string(),
        };
        assert_eq!(
            no_admin.into_wasm_msg(Addr::unchecked("ekez")),
            WasmMsg::Instantiate {
                admin: None,
                code_id: 42,
                msg: to_json_binary("foo").unwrap(),
                funds: vec![],
                label: "bar".to_string()
            }
        )
    }

    #[test]
    fn test_instantiate_admin_addr() {
        let no_admin = ContractInstantiateInfo {
            code_id: 42,
            msg: to_json_binary("foo").unwrap(),
            admin: Some(Admin::Address {
                addr: "core".to_string(),
            }),
            label: "bar".to_string(),
        };
        assert_eq!(
            no_admin.into_wasm_msg(Addr::unchecked("ekez")),
            WasmMsg::Instantiate {
                admin: Some("core".to_string()),
                code_id: 42,
                msg: to_json_binary("foo").unwrap(),
                funds: vec![],
                label: "bar".to_string()
            }
        )
    }

    #[test]
    fn test_instantiate_instantiator_addr() {
        let no_admin = ContractInstantiateInfo {
            code_id: 42,
            msg: to_json_binary("foo").unwrap(),
            admin: Some(Admin::Instantiator {}),
            label: "bar".to_string(),
        };
        assert_eq!(
            no_admin.into_wasm_msg(Addr::unchecked("ekez")),
            WasmMsg::Instantiate {
                admin: Some("ekez".to_string()),
                code_id: 42,
                msg: to_json_binary("foo").unwrap(),
                funds: vec![],
                label: "bar".to_string()
            }
        )
    }
}
