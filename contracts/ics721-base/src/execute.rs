use cosmwasm_std::{Addr, DepsMut, StdResult};
use ics721::{execute::Ics721Execute, state::CollectionData, utils::get_collection_data};

use crate::state::Ics721Contract;

impl Ics721Execute for Ics721Contract {
    type ClassData = CollectionData;

    /// Default ics721-base contract collections collection data from cw721 contract.
    fn get_class_data(&self, deps: &DepsMut, sender: &Addr) -> StdResult<Option<Self::ClassData>> {
        get_collection_data(deps, sender).map(Option::Some)
    }
}
