use snip2pin::permission::ScreenPermission;
use std::cell::Cell;

#[test]
fn granted_access_never_prompts_and_revocation_stops_capture() {
    let mut permission = ScreenPermission::default();
    assert!(permission.authorize(|| true, || panic!("already authorized")));
    assert_eq!(permission, ScreenPermission::Granted);
    assert!(!permission.authorize(|| false, || panic!("do not repeat prompts")));
    assert_eq!(permission, ScreenPermission::Required);
}

#[test]
fn denial_does_not_loop_and_a_later_user_action_observes_settings_changes() {
    let granted = Cell::new(false);
    let prompts = Cell::new(0);
    let caller = std::thread::current().id();
    let mut permission = ScreenPermission::default();
    assert!(!permission.authorize(
        || granted.get(),
        || {
            assert_eq!(std::thread::current().id(), caller);
            prompts.set(prompts.get() + 1);
            false
        },
    ));
    assert_eq!(permission, ScreenPermission::Required);
    assert!(!permission.authorize(|| granted.get(), || panic!("no repeated dialog")));
    assert_eq!(prompts.get(), 1);
    granted.set(true);
    assert!(permission.authorize(|| granted.get(), || panic!("settings already granted")));
    assert_eq!(permission, ScreenPermission::Granted);
}

#[test]
fn first_prompt_only_proceeds_when_preflight_confirms_access() {
    let granted = Cell::new(false);
    let mut permission = ScreenPermission::default();
    assert!(permission.authorize(
        || granted.get(),
        || {
            granted.set(true);
            true
        },
    ));
    let mut permission = ScreenPermission::default();
    assert!(
        !permission.authorize(|| false, || true),
        "a dialog result alone cannot authorize capture"
    );
}
