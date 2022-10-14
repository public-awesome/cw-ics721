//! This provides a simple type, `PauseOrchestrator`, that allows a
//! specified address to execute a pause a single time.

use cosmwasm_std::{Addr, Api, StdError, StdResult, Storage};
use cw_storage_plus::Item;
use thiserror::Error;

#[cfg(test)]
mod tests;

#[derive(Error, Debug, PartialEq)]
pub enum PauseError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("contract is paused pending governance intervention")]
    Paused {},

    #[error("unauthorized pauser ({sender})")]
    Unauthorized { sender: Addr },
}

pub struct PauseOrchestrator<'a> {
    pauser: Item<'a, Option<Addr>>,
    paused: Item<'a, bool>,
}

impl<'a> PauseOrchestrator<'a> {
    /// Creates a new pause orchestrator using the provided storage
    /// keys.
    pub const fn new(pauser_key: &'a str, paused_key: &'a str) -> Self {
        Self {
            pauser: Item::new(pauser_key),
            paused: Item::new(paused_key),
        }
    }

    /// Sets a new pauser who may pause the contract. If the contract
    /// is paused, it is unpaused.
    pub fn set_pauser(
        &self,
        storage: &mut dyn Storage,
        api: &dyn Api,
        pauser: Option<&str>,
    ) -> StdResult<()> {
        self.pauser
            .save(storage, &pauser.map(|h| api.addr_validate(h)).transpose()?)?;
        self.paused.save(storage, &false)
    }

    /// Errors if the module is paused, does nothing otherwise.
    pub fn error_if_paused(&self, storage: &dyn Storage) -> Result<(), PauseError> {
        if self.paused.load(storage)? {
            Err(PauseError::Paused {})
        } else {
            Ok(())
        }
    }

    /// Pauses the module and removes the previous pauser's ability to
    /// pause.
    pub fn pause(&self, storage: &mut dyn Storage, sender: &Addr) -> Result<(), PauseError> {
        self.error_if_paused(storage)?;

        let pauser = self.pauser.load(storage)?;
        if pauser.as_ref().map_or(true, |pauser| sender != pauser) {
            Err(PauseError::Unauthorized {
                sender: sender.clone(),
            })
        } else {
            self.paused.save(storage, &true)?;
            Ok(())
        }
    }

    /// Gets the pause policy for this orchestrator. If there is no
    /// pause policy (the orchestrator may not be paused), returns
    /// None.
    pub fn query_pauser(&self, storage: &dyn Storage) -> StdResult<Option<Addr>> {
        self.pauser.load(storage)
    }

    /// Gets when this orchestrator will unpause. If the orchestrator
    /// is not paused, returns None.
    pub fn query_paused(&self, storage: &dyn Storage) -> StdResult<bool> {
        self.paused.load(storage)
    }
}
