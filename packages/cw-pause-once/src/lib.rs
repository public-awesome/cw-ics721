//! This provides a simple type, `PauseOrchestrator`, that allows a
//! specified address to execute a pause a single time and pause for a
//! prespecified duration.
//!
//! This might be useful if you want to delegate the ability to pause
//! a contract to an address, while also not allowing that address to
//! perminantly lock the contract. For example, you may want to set
//! the prespecified duration to slightly over one governance cycle
//! for SDK governance, and then set a small subDAO as the
//! pauser. This way the subDAO may pause the contract quickly, but
//! must be reauthorized by governance to do it again.

use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Api, BlockInfo, StdError, StdResult, Storage};
use cw_storage_plus::Item;
use cw_utils::{Duration, Expiration};
use thiserror::Error;

#[cfg(test)]
mod tests;

#[derive(Error, Debug, PartialEq)]
pub enum PauseError {
    #[error(transparent)]
    Std(#[from] StdError),

    #[error("paused. unpause scheduled for ({expiration})")]
    Paused { expiration: Expiration },

    #[error("unauthorized pauser ({sender})")]
    Unauthorized { sender: Addr },
}

#[cw_serde]
pub struct UncheckedPausePolicy {
    pauser: String,
    pause_duration: Duration,
}

#[cw_serde]
pub struct PausePolicy {
    pauser: Addr,
    pause_duration: Duration,
}

pub struct PauseOrchestrator<'a> {
    policy: Item<'a, Option<PausePolicy>>,
    paused_until: Item<'a, Option<Expiration>>,
}

impl UncheckedPausePolicy {
    pub fn into_checked(self, api: &dyn Api) -> StdResult<PausePolicy> {
        Ok(PausePolicy {
            pauser: api.addr_validate(&self.pauser)?,
            pause_duration: self.pause_duration,
        })
    }
}

impl<'a> PauseOrchestrator<'a> {
    /// Creates a new pause orchestrator using the provided storage
    /// keys.
    pub const fn new(policy_key: &'a str, paused_key: &'a str) -> Self {
        Self {
            policy: Item::new(policy_key),
            paused_until: Item::new(paused_key),
        }
    }

    /// Sets a new pause policy and resets any old pauses that may be
    /// present. This must be called at least once per instance of
    /// `PauseOrchestrator`.
    pub fn set_policy(
        &self,
        storage: &mut dyn Storage,
        policy: Option<PausePolicy>,
    ) -> StdResult<()> {
        self.policy.save(storage, &policy)?;
        self.paused_until.save(storage, &None)
    }

    /// Errors if the module is paused, does nothing otherwise.
    pub fn error_if_paused(
        &self,
        storage: &dyn Storage,
        block: &BlockInfo,
    ) -> Result<(), PauseError> {
        self.paused_until
            .load(storage)?
            .map_or(Ok(()), |expiration| {
                if !expiration.is_expired(block) {
                    Err(PauseError::Paused { expiration })
                } else {
                    Ok(())
                }
            })
    }

    /// Pauses the module and removes the previous pauser's ability to
    /// pause.
    pub fn pause(
        &self,
        storage: &mut dyn Storage,
        sender: &Addr,
        block: &BlockInfo,
    ) -> Result<(), PauseError> {
        self.error_if_paused(storage, block)?;

        let policy = self.policy.load(storage)?;
        if let Some(PausePolicy {
            pauser,
            pause_duration,
        }) = policy
        {
            if pauser == *sender {
                self.policy.save(storage, &None)?;
                self.paused_until
                    .save(storage, &Some(pause_duration.after(block)))?;
                return Ok(());
            }
        }
        Err(PauseError::Unauthorized {
            sender: sender.clone(),
        })
    }

    /// Gets the pause policy for this orchestrator. If there is no
    /// pause policy (the orchestrator may not be paused), returns
    /// None.
    pub fn query_pause_policy(&self, storage: &mut dyn Storage) -> StdResult<Option<PausePolicy>> {
        self.policy.load(storage)
    }

    /// Gets when this orchestrator will unpause. If the orchestrator
    /// is not paused, returns None.
    pub fn query_paused_until(
        &self,
        storage: &mut dyn Storage,
        block: &BlockInfo,
    ) -> StdResult<Option<Expiration>> {
        let paused_until = self.paused_until.load(storage)?;
        Ok(paused_until
            .map(|paused_until| {
                if paused_until.is_expired(block) {
                    None
                } else {
                    Some(paused_until)
                }
            })
            .flatten())
    }
}
