//! What changed in a window between two snapshots.
//!
//! Every snapshot leaves a fingerprint file beside its refmap: one line per node with
//! its role and a hash of its name, value, description and states, never the text
//! itself. `snapshot --diff <id>` compares a new tree against that file and reports the
//! nodes that appeared, with their text and refs, and how many nodes of each role are
//! gone. A node whose text or states changed counts as one gone and one new node. Both
//! snapshots must use the same view (depth, `-i`, `--compact`, surface): the result says
//! so in `same_view`, because a different view changes which nodes are listed.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{AccessibilityNode, AdapterError, AppError, ErrorCode, refs::refs_store::RefStore};

const FINGERPRINT_FILE: &str = "fingerprints.json";
const MAX_LINES: usize = 20_000;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_REPORTED: usize = 60;

#[derive(Serialize, Deserialize)]
struct FingerprintFile {
    app: String,
    window_id: String,
    #[serde(default)]
    view: String,
    complete: bool,
    lines: Vec<(String, String)>,
}

/// The window a snapshot read, as the diff compares it.
pub(crate) struct DiffWindow<'a> {
    pub(crate) app: &'a str,
    pub(crate) window_id: &'a str,
    pub(crate) view: String,
    pub(crate) complete: bool,
}

/// The tree options that decide which nodes a snapshot lists, as one comparable string.
pub(crate) fn view_of(options: &crate::adapter::TreeOptions) -> String {
    format!(
        "depth={} interactive={} compact={} skeleton={} surface={:?}",
        options.max_depth,
        options.interactive_only,
        options.compact,
        options.skeleton,
        options.surface
    )
}

/// Saves the fingerprints of `tree` beside the refmap of `snapshot_id`.
pub(crate) fn save_fingerprints(
    store: &RefStore,
    snapshot_id: &str,
    tree: &AccessibilityNode,
    window: &DiffWindow<'_>,
) -> Result<(), AppError> {
    let mut lines = Vec::new();
    walk(tree, &mut |node| {
        if lines.len() < MAX_LINES {
            lines.push((node.role.clone(), fingerprint(node)));
        }
    });
    let file = FingerprintFile {
        app: window.app.to_string(),
        window_id: window.window_id.to_string(),
        view: window.view.clone(),
        complete: window.complete && lines.len() < MAX_LINES,
        lines,
    };
    let path = store.snapshot_dir(snapshot_id)?.join(FINGERPRINT_FILE);
    crate::storage::private_file::write_atomic(&path, &serde_json::to_vec(&file)?)?;
    Ok(())
}

/// The `diff` block of a snapshot: `tree` against the saved fingerprints of
/// `baseline_id`.
pub(crate) fn diff_against(
    store: &RefStore,
    baseline_id: &str,
    tree: &AccessibilityNode,
    window: &DiffWindow<'_>,
) -> Result<Value, AppError> {
    let baseline = load(&store.snapshot_dir(baseline_id)?, baseline_id)?;
    let mut remaining: HashMap<(String, String), usize> = HashMap::new();
    for line in &baseline.lines {
        *remaining.entry(line.clone()).or_default() += 1;
    }
    let mut added = Vec::new();
    let mut added_count = 0_usize;
    let mut unchanged = 0_usize;
    walk(tree, &mut |node| {
        let key = (node.role.clone(), fingerprint(node));
        match remaining.get_mut(&key) {
            Some(count) if *count > 0 => {
                *count -= 1;
                unchanged += 1;
            }
            _ => {
                added_count += 1;
                if added.len() < MAX_REPORTED {
                    added.push(describe(node));
                }
            }
        }
    });
    let mut removed_by_role: BTreeMap<String, usize> = BTreeMap::new();
    for ((role, _), count) in remaining.into_iter().filter(|(_, count)| *count > 0) {
        *removed_by_role.entry(role).or_default() += count;
    }
    let removed_count: usize = removed_by_role.values().sum();
    Ok(json!({
        "baseline": baseline_id,
        "baseline_complete": baseline.complete,
        "same_window": baseline.app == window.app && baseline.window_id == window.window_id,
        "same_view": baseline.view == window.view,
        "complete": window.complete,
        "unchanged": unchanged,
        "added_count": added_count,
        "added": added,
        "added_truncated": added_count > MAX_REPORTED,
        "removed_count": removed_count,
        "removed_by_role": removed_by_role,
    }))
}

fn load(dir: &Path, baseline_id: &str) -> Result<FingerprintFile, AppError> {
    if !dir.join("refmap.json").exists() {
        return Err(AdapterError::snapshot_not_found(baseline_id).into());
    }
    let bytes = match crate::storage::private_file::read_private_bounded(
        &dir.join(FINGERPRINT_FILE),
        MAX_FILE_BYTES,
    ) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AdapterError::new(
                ErrorCode::InvalidArgs,
                format!("Snapshot '{baseline_id}' has no fingerprints to compare against"),
            )
            .with_suggestion(
                "Take the baseline snapshot with this version of agent-desktop, act, then run snapshot --diff with its id.",
            )
            .into());
        }
        Err(error) => return Err(error.into()),
    };
    Ok(serde_json::from_slice(&bytes)?)
}

fn walk(node: &AccessibilityNode, visit: &mut impl FnMut(&AccessibilityNode)) {
    visit(node);
    for child in &node.children {
        walk(child, visit);
    }
}

fn describe(node: &AccessibilityNode) -> Value {
    let mut entry = json!({ "role": node.role });
    if let Some(name) = &node.identity.name {
        entry["name"] = json!(name);
    }
    if let Some(value) = &node.identity.value {
        entry["value"] = json!(value);
    }
    if !node.presentation.states.is_empty() {
        entry["states"] = json!(node.presentation.states);
    }
    if let Some(ref_id) = &node.ref_id {
        entry["ref_id"] = json!(ref_id);
    }
    entry
}

/// A stable FNV-1a hash of what a person reads on the node and its states. The same
/// text always gives the same hash, across runs and builds.
fn fingerprint(node: &AccessibilityNode) -> String {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET;
    let mut feed = |text: &str| {
        for byte in text.as_bytes().iter().chain(std::iter::once(&0x1f)) {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(PRIME);
        }
    };
    let mut states = node.presentation.states.clone();
    states.sort();
    feed(&node.role);
    feed(node.identity.name.as_deref().unwrap_or_default());
    feed(node.identity.value.as_deref().unwrap_or_default());
    feed(node.identity.description.as_deref().unwrap_or_default());
    feed(&states.join("\u{1e}"));
    format!("{hash:016x}")
}

#[cfg(test)]
#[path = "tree_diff_tests.rs"]
mod tests;
