use super::*;

#[test]
fn given_state_dir_when_built_then_ends_with_app_name() {
    assert_eq!(
        state_dir().file_name().and_then(|n| n.to_str()),
        Some("tacet")
    );
}

#[test]
fn given_guard_when_dropped_then_no_panic_and_child_handled() {
    let guard = prevent_sleep();
    let had_child = guard.child.is_some();
    drop(guard);
    assert_eq!(had_child, cfg!(target_os = "macos"));
}
