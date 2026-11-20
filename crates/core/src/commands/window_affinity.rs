//! Window affinity: the window a caller last snapshotted for an application.
//!
//! A command that names only an application resolves the application's focused
//! window, so a person who brings another window of the same application to
//! the front moves every later command into their window, and with no window
//! focused the choice is ambiguous. Affinity keeps naming the remembered
//! window while it exists, per session and agent, so the caller keeps working
//! in its own window behind the person's.

use crate::{AppError, WindowInfo, context::CommandContext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const FILE_NAME: &str = "window_affinity.json";
const MAX_BYTES: u64 = 256 * 1024;
const MAX_AGE_SECS: u64 = 12 * 60 * 60;
const MAX_ENTRIES: usize = 64;

/// The remembered window for one application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Affinity {
    pub(crate) window_id: String,
    #[serde(default)]
    pub(crate) process_instance: Option<String>,
    pub(crate) updated_unix: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct AffinityFile {
    #[serde(default)]
    entries: BTreeMap<String, Affinity>,
}

/// Returns the remembered window when the command names an application, no
/// window, and the window surface, which is the only case affinity applies to.
pub(crate) fn sticky_target(
    context: &CommandContext,
    app: Option<&str>,
    window_id: Option<&str>,
    window_surface: bool,
) -> Option<Affinity> {
    if window_id.is_some() || !window_surface {
        return None;
    }
    let path = file_path(context)?;
    recall_at(&path, &key(context.agent_id(), app?), now_unix())
}

/// Records the window a snapshot observed, under the application name the
/// caller typed and under the window's own application name.
pub(crate) fn remember(context: &CommandContext, app: Option<&str>, window: &WindowInfo) {
    let Some(path) = file_path(context) else {
        return;
    };
    let entry = Affinity {
        window_id: window.id.clone(),
        process_instance: window.process_instance.clone(),
        updated_unix: now_unix(),
    };
    let mut keys = vec![key(context.agent_id(), &window.app)];
    if let Some(app) = app {
        let typed = key(context.agent_id(), app);
        if !keys.contains(&typed) {
            keys.push(typed);
        }
    }
    let mut file = load(&path);
    merge(&mut file, &keys, &entry);
    if let Ok(bytes) = serde_json::to_vec(&file) {
        let _ = crate::storage::private_file::write_atomic(&path, &bytes);
    }
}

/// Drops the remembered window for `app`, after it stopped resolving.
pub(crate) fn forget(context: &CommandContext, app: Option<&str>) {
    let (Some(path), Some(app)) = (file_path(context), app) else {
        return;
    };
    let mut file = load(&path);
    if file.entries.remove(&key(context.agent_id(), app)).is_some()
        && let Ok(bytes) = serde_json::to_vec(&file)
    {
        let _ = crate::storage::private_file::write_atomic(&path, &bytes);
    }
}

/// True when the remembered window no longer resolves and the command should
/// fall back to the ordinary window choice.
pub(crate) fn window_vanished(error: &AppError) -> bool {
    matches!(error, AppError::Adapter(error) if error.code == crate::ErrorCode::WindowNotFound)
}

/// Runs `attempt` against the remembered window, if any, and falls back to the
/// window the caller named (or none) when the remembered one is gone.
pub(crate) fn with_sticky_window<T>(
    context: &CommandContext,
    app: Option<&str>,
    window_id: Option<&str>,
    window_surface: bool,
    mut attempt: impl FnMut(Option<&str>) -> Result<T, AppError>,
) -> Result<(T, Option<Affinity>), AppError> {
    let sticky = sticky_target(context, app, window_id, window_surface);
    let Some(affinity) = sticky else {
        return attempt(window_id).map(|value| (value, None));
    };
    match attempt(Some(affinity.window_id.as_str())) {
        Err(error) if window_vanished(&error) => {
            forget(context, app);
            attempt(window_id).map(|value| (value, None))
        }
        result => result.map(|value| (value, Some(affinity))),
    }
}

/// Describes a kept window in the response, so the caller knows why it did
/// not get the application's focused window and how to choose another.
pub(crate) fn kept_note(affinity: &Affinity, focused: bool) -> Option<Value> {
    if focused {
        return None;
    }
    Some(json!({
        "window_id": affinity.window_id,
        "kept": true,
        "note": "Kept the window this caller last snapshotted for this app, not the app's focused window; pass --window-id to target another window.",
    }))
}

fn merge(file: &mut AffinityFile, keys: &[String], entry: &Affinity) {
    file.entries
        .retain(|_, existing| is_fresh(existing, entry.updated_unix));
    for key in keys {
        file.entries.insert(key.clone(), entry.clone());
    }
    while file.entries.len() > MAX_ENTRIES {
        let Some(oldest) = file
            .entries
            .iter()
            .min_by_key(|(_, existing)| existing.updated_unix)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        file.entries.remove(&oldest);
    }
}

fn recall_at(path: &Path, key: &str, now: u64) -> Option<Affinity> {
    load(path)
        .entries
        .get(key)
        .filter(|entry| is_fresh(entry, now))
        .cloned()
}

fn is_fresh(entry: &Affinity, now: u64) -> bool {
    now.saturating_sub(entry.updated_unix) <= MAX_AGE_SECS
}

fn key(agent: Option<&str>, app: &str) -> String {
    format!("{}|{}", agent.unwrap_or("-"), app.trim().to_lowercase())
}

fn load(path: &Path) -> AffinityFile {
    crate::storage::private_file::read_private_bounded(path, MAX_BYTES)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn file_path(context: &CommandContext) -> Option<PathBuf> {
    let directory = match context.session_id() {
        Some(session) => crate::session::session_dir(session).ok()?,
        None => crate::session::agent_desktop_dir().ok()?,
    };
    Some(directory.join(FILE_NAME))
}

fn now_unix() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "window_affinity_tests.rs"]
mod tests;
