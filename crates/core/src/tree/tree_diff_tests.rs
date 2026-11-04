use super::*;
use crate::refs::{RefMap, refs_test_support::HomeGuard};

fn node(role: &str, name: &str, children: Vec<AccessibilityNode>) -> AccessibilityNode {
    AccessibilityNode {
        ref_id: None,
        role: role.into(),
        identity: crate::NodeIdentity {
            retained_object: None,
            name: Some(name.into()),
            value: None,
            description: None,
            native_id: None,
        },
        presentation: Default::default(),
        children_count: None,
        subtree_truncated: false,
        children,
    }
}

fn calculator(display: &str) -> AccessibilityNode {
    node(
        "window",
        "Calculator",
        vec![
            node("statictext", display, vec![]),
            node("button", "Equals", vec![]),
        ],
    )
}

fn window(window_id: &str) -> DiffWindow<'_> {
    DiffWindow {
        app: "Calculator",
        window_id,
        view: "depth=10 interactive=true".into(),
        complete: true,
    }
}

fn saved_baseline(store: &RefStore, tree: &AccessibilityNode) -> String {
    let snapshot_id = store.save_new_snapshot(&RefMap::new()).unwrap();
    save_fingerprints(store, &snapshot_id, tree, &window("w-1")).unwrap();
    snapshot_id
}

#[test]
fn a_diff_reports_the_new_text_and_counts_what_is_gone() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let baseline = saved_baseline(&store, &calculator("0"));

    let diff = diff_against(&store, &baseline, &calculator("42"), &window("w-1")).unwrap();

    assert_eq!(diff["baseline"], baseline.as_str());
    assert_eq!(diff["same_window"], true);
    assert_eq!(diff["unchanged"], 2);
    assert_eq!(diff["added_count"], 1);
    assert_eq!(diff["added"][0]["role"], "statictext");
    assert_eq!(diff["added"][0]["name"], "42");
    assert_eq!(diff["removed_count"], 1);
    assert_eq!(diff["removed_by_role"]["statictext"], 1);
}

#[test]
fn the_saved_fingerprints_hold_no_text_from_the_screen() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let baseline = saved_baseline(&store, &calculator("secret-balance-1234"));

    let saved = std::fs::read_to_string(
        store
            .snapshot_dir(&baseline)
            .unwrap()
            .join(FINGERPRINT_FILE),
    )
    .unwrap();

    assert!(!saved.contains("secret-balance-1234"));
    assert!(!saved.contains("Equals"));
}

#[test]
fn an_unchanged_window_has_an_empty_diff_and_another_window_is_flagged() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let baseline = saved_baseline(&store, &calculator("7"));

    let same = diff_against(&store, &baseline, &calculator("7"), &window("w-1")).unwrap();
    let other = diff_against(&store, &baseline, &calculator("7"), &window("w-2")).unwrap();

    assert_eq!(same["added_count"], 0);
    assert_eq!(same["removed_count"], 0);
    assert_eq!(same["same_view"], true);
    assert_eq!(other["same_window"], false);

    let mut full_tree = window("w-1");
    full_tree.view = "depth=10 interactive=false".into();
    let other_view = diff_against(&store, &baseline, &calculator("7"), &full_tree).unwrap();
    assert_eq!(other_view["same_view"], false);
}

#[test]
fn a_missing_baseline_or_one_without_fingerprints_fails_clearly() {
    let _home = HomeGuard::new();
    let store = RefStore::new().unwrap();
    let missing =
        diff_against(&store, "s404missing", &calculator("1"), &window("w-1")).unwrap_err();
    let older = store.save_new_snapshot(&RefMap::new()).unwrap();
    let without = diff_against(&store, &older, &calculator("1"), &window("w-1")).unwrap_err();

    assert_eq!(missing.code(), "SNAPSHOT_NOT_FOUND");
    assert_eq!(without.code(), "INVALID_ARGS");
    assert!(without.suggestion().is_some_and(|s| s.contains("--diff")));
}
