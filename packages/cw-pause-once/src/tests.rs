use cosmwasm_std::{
    testing::{mock_dependencies, mock_env},
    Addr,
};
use cw_utils::{Duration, Expiration};

use crate::{PauseError, PauseOrchestrator, UncheckedPausePolicy};

#[test]
fn test_pause() {
    let mut deps = mock_dependencies();
    let mut env = mock_env();

    let policy = UncheckedPausePolicy {
        pauser: "ekez".to_string(),
        pause_duration: Duration::Height(1),
    };

    let po = PauseOrchestrator::new("policy", "pause");
    po.set_policy(
        &mut deps.storage,
        Some(policy.into_checked(&deps.api).unwrap()),
    )
    .unwrap();

    // Only the designated pauser may pause.
    let err: PauseError = po
        .pause(&mut deps.storage, &Addr::unchecked("notekez"), &env.block)
        .unwrap_err();
    assert_eq!(
        err,
        PauseError::Unauthorized {
            sender: Addr::unchecked("notekez")
        }
    );
    // Not paused.
    po.error_if_paused(&deps.storage, &env.block).unwrap();

    po.pause(&mut deps.storage, &Addr::unchecked("ekez"), &env.block)
        .unwrap();

    let err: PauseError = po.error_if_paused(&deps.storage, &env.block).unwrap_err();
    assert_eq!(
        err,
        PauseError::Paused {
            expiration: Expiration::AtHeight(env.block.height + 1)
        }
    );

    // Time's arrow merely marches forward.
    env.block.height += 1;

    // No longer paused.
    po.error_if_paused(&deps.storage, &env.block).unwrap();
    // Original pauser burned their pause privledges.
    let err: PauseError = po
        .pause(&mut deps.storage, &Addr::unchecked("ekez"), &env.block)
        .unwrap_err();
    assert_eq!(
        err,
        PauseError::Unauthorized {
            sender: Addr::unchecked("ekez")
        }
    );

    // Assign a new pauser.
    let policy = UncheckedPausePolicy {
        pauser: "meow".to_string(),
        pause_duration: Duration::Time(100),
    };
    po.set_policy(
        &mut deps.storage,
        Some(policy.into_checked(&deps.api).unwrap()),
    )
    .unwrap();

    // New pauser can pause.
    po.pause(&mut deps.storage, &Addr::unchecked("meow"), &env.block)
        .unwrap();

    let err: PauseError = po.error_if_paused(&deps.storage, &env.block).unwrap_err();
    assert_eq!(
        err,
        PauseError::Paused {
            expiration: Expiration::AtTime(env.block.time.plus_seconds(100))
        }
    );

    // Removing the pauser removes their pause as well.
    po.set_policy(&mut deps.storage, None).unwrap();
    po.error_if_paused(&deps.storage, &env.block).unwrap();

    // With no policy set, attempting to pause fails.
    let err: PauseError = po
        .pause(&mut deps.storage, &Addr::unchecked("meow"), &env.block)
        .unwrap_err();
    assert_eq!(
        err,
        PauseError::Unauthorized {
            sender: Addr::unchecked("meow")
        }
    );
}
