use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
pub struct InstantiateMsg {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExecuteMsg {
    /// Transfer the NFT identified by class_id and token_id to receiver
    Transfer {
        class_id: String,
        token_id: String,
        receiver: String,
    },
    /// Burn the NFT identified by class_id and token_id
    Burn { class_id: String, token_id: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QueryMsg {
    /// Returns the current owner of the NFT identified by class_id and token_id
    GetOwner { token_id: String, class_id: String },
    /// Returns the NFT identified by class_id and token_id
    GetNft { class_id: String, token_id: String },
    /// Returns true if the NFT class identified by class_id already
    /// exists
    HasClass { class_id: String },
    /// Returns the NFT Class identified by class_id
    GetClass { class_id: String },
}
