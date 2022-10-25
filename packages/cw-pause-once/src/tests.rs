use cosmwasm_std::{testing::mock_dependencies, Addr};

use crate::{PauseError, PauseOrchestrator};

#[test]
fn test_pause() {
    let mut deps = mock_dependencies();
    let storage = &mut deps.storage;
    let api = &deps.api;

    let pauser = PauseOrchestrator::new("pauser", "paused");
    pauser.set_pauser(storage, api, Some("ekez")).unwrap();

    // Should start unpaused.
    let paused = pauser.query_paused(storage).unwrap();
    assert!(!paused);

    // Non-pauser can not pause.
    let err = pauser.pause(storage, &Addr::unchecked("zeke")).unwrap_err();
    assert_eq!(
        err,
        PauseError::Unauthorized {
            sender: Addr::unchecked("zeke")
        }
    );

    // Pauser can pause once.
    pauser.pause(storage, &Addr::unchecked("ekez")).unwrap();
    let paused = pauser.query_paused(storage).unwrap();
    assert!(paused);

    let err = pauser.pause(storage, &Addr::unchecked("ekez")).unwrap_err();
    assert_eq!(err, PauseError::Paused {});

    // Nominate a new pauser.
    pauser.set_pauser(storage, api, Some("zeke")).unwrap();

    // Nomination unpauses.
    let paused = pauser.query_paused(storage).unwrap();
    assert!(!paused);

    // Old pauser may not pause.
    let err = pauser.pause(storage, &Addr::unchecked("ekez")).unwrap_err();
    assert_eq!(
        err,
        PauseError::Unauthorized {
            sender: Addr::unchecked("ekez")
        }
    );

    // New pauser may pause.
    pauser.pause(storage, &Addr::unchecked("zeke")).unwrap();
    let paused = pauser.query_paused(storage).unwrap();
    assert!(paused);
}
