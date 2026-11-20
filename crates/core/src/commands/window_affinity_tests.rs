use super::{Affinity, AffinityFile, MAX_AGE_SECS, MAX_ENTRIES, is_fresh, kept_note, key, merge};

fn entry(window_id: &str, updated_unix: u64) -> Affinity {
    Affinity {
        window_id: window_id.into(),
        process_instance: Some("macos-proc-v1:1:2".into()),
        updated_unix,
    }
}

#[test]
fn keys_separate_agents_and_ignore_app_name_case() {
    assert_eq!(key(None, " Microsoft Edge "), key(None, "microsoft edge"));
    assert_ne!(key(Some("a1"), "Safari"), key(Some("a2"), "Safari"));
    assert_ne!(key(None, "Safari"), key(Some("a1"), "Safari"));
}

#[test]
fn a_new_snapshot_replaces_the_remembered_window() {
    let mut file = AffinityFile::default();
    let keys = vec![key(None, "Edge")];
    merge(&mut file, &keys, &entry("w-1", 100));
    merge(&mut file, &keys, &entry("w-2", 200));
    assert_eq!(file.entries[&keys[0]].window_id, "w-2");
}

#[test]
fn stale_entries_expire_and_the_file_stays_bounded() {
    let mut file = AffinityFile::default();
    let now = 10 * MAX_AGE_SECS;
    merge(&mut file, &[key(None, "old")], &entry("w-old", 1));
    for index in 0..(MAX_ENTRIES + 10) {
        merge(
            &mut file,
            &[key(None, &format!("app{index}"))],
            &entry("w", now + index as u64),
        );
    }
    assert!(!file.entries.contains_key(&key(None, "old")));
    assert_eq!(file.entries.len(), MAX_ENTRIES);
    assert!(
        file.entries
            .contains_key(&key(None, &format!("app{}", MAX_ENTRIES + 9)))
    );
}

#[test]
fn freshness_follows_the_age_limit() {
    let remembered = entry("w-1", 1_000);
    assert!(is_fresh(&remembered, 1_000 + MAX_AGE_SECS));
    assert!(!is_fresh(&remembered, 1_001 + MAX_AGE_SECS));
}

#[test]
fn the_note_appears_only_when_the_kept_window_is_not_focused() {
    let remembered = entry("w-1", 1);
    assert!(kept_note(&remembered, true).is_none());
    let note = kept_note(&remembered, false).expect("note for an unfocused kept window");
    assert_eq!(note["window_id"], "w-1");
    assert_eq!(note["kept"], true);
}
