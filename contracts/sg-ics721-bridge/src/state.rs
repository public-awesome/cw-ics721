use cosmwasm_std::Addr;
use cw_pause_once::PauseOrchestrator;
use cw_storage_plus::Item;
use ics721::{
    state::{ChannelsInfo, ClassIdInfo, Cw721Info},
    Ics721Contract,
};

// This type is an exact copy of Ics721Contract, since only traits defined in the current crate
// can be implemented for types defined outside of the crate.
pub struct SgIcs721Contract<'a> {
    /// The proxy that this contract is receiving NFTs from, if any.
    pub proxy: Item<'a, Option<Addr>>,
    /// Manages contract pauses.
    pub po: PauseOrchestrator<'a>,
    /// cw721 related info like code ID and token metadata.
    pub cw721_info: Cw721Info<'a>,
    /// cw721 class related info like class ID to cw721 contract mappings.
    pub class_id_info: ClassIdInfo<'a>,
    /// Maps (class ID, token ID) -> to local incoming and outgoing channel ID.
    pub channels_info: ChannelsInfo<'a>,
}

impl Default for SgIcs721Contract<'static> {
    fn default() -> Self {
        let Ics721Contract {
            proxy,
            po,
            cw721_info,
            class_id_info,
            channels_info,
        } = Ics721Contract::default();
        Self {
            proxy,
            po,
            cw721_info,
            class_id_info,
            channels_info,
        }
    }
}
