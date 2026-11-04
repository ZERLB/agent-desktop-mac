use crate::{
    AppError,
    adapter::{PlatformAdapter, SnapshotSurface},
    commands::{wait_selector, wait_selector::WaitSelectorInput},
    context::CommandContext,
    refs::validate_ref_id,
    snapshot,
    snapshot::snapshot_ref,
};
use serde_json::{Value, json};

pub struct SnapshotArgs {
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub max_depth: u8,
    pub include_bounds: bool,
    pub interactive_only: bool,
    pub compact: bool,
    pub surface: SnapshotSurface,
    pub skeleton: bool,
    pub root_ref: Option<String>,
    /// Snapshot deadline in milliseconds. A16-11 measured a cold Chromium
    /// settle at 10-25 s against a hardcoded 3 s deadline, so the caller can
    /// raise it when the post-settle still-thin guidance names
    /// `--timeout-ms`.
    pub timeout_ms: Option<u64>,
    /// Parenthetical spelling the observation-mode sub-struct: the caller says
    /// Chromium accessibility is or will be forced, so the adapter does not
    /// need to guess at guidance. Threaded into the request's observation
    /// mode.
    pub force_electron_a11y: bool,
    /// An earlier snapshot of the same window to compare against: the result then
    /// carries a `diff` block with the nodes that appeared and how many are gone.
    pub diff_from: Option<String>,
}

fn tree_options(args: &SnapshotArgs) -> crate::adapter::TreeOptions {
    let skeleton_applies = args.skeleton && args.root_ref.is_none();
    let effective_depth = if skeleton_applies {
        args.max_depth.min(3)
    } else {
        args.max_depth
    };

    crate::adapter::TreeOptions {
        max_depth: effective_depth,
        include_bounds: args.include_bounds,
        interactive_only: args.interactive_only,
        compact: args.compact,
        surface: args.surface,
        skeleton: skeleton_applies,
        force_renderer_accessibility: args.force_electron_a11y,
    }
}

pub fn execute(
    args: SnapshotArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    tracing::debug!(
        max_depth = args.max_depth,
        interactive_only = args.interactive_only,
        compact = args.compact,
        "tree: snapshot"
    );

    if args.diff_from.is_some() && (args.root_ref.is_some() || context.wait_selector().is_some()) {
        return Err(AppError::invalid_input_with_suggestion(
            "--diff cannot be combined with --root, --wait-for or --wait-for-gone",
            "Run snapshot --diff <snapshot_id> on the whole window, without those flags.",
        ));
    }

    if let Some(root) = args.root_ref.as_deref() {
        if context.wait_selector().is_some() {
            return Err(AppError::invalid_input_with_suggestion(
                "--root cannot be combined with --wait-for or --wait-for-gone",
                "Run snapshot without --root, or omit the wait selector flags.",
            ));
        }
        super::surface_scope::reject_root_with_surface("snapshot", Some(root), args.surface)?;
        validate_ref_id(root)?;
    }

    adapter.check_surface_support(args.surface)?;

    let opts = tree_options(&args);

    if let Some(root) = args.root_ref {
        return format_result(snapshot_ref::run_from_ref_with_context(
            adapter,
            &opts,
            &root,
            context,
            args.timeout_ms
                .unwrap_or(snapshot::DEFAULT_SNAPSHOT_TIMEOUT_MS),
        )?);
    }

    if let Some(wait) = context.wait_selector() {
        return wait_selector::execute(
            WaitSelectorInput {
                query_raw: wait.query_raw.clone(),
                gone: wait.gone,
                app: args.app.clone(),
                window_id: args.window_id.clone(),
                opts,
                timeout_ms: wait.timeout_ms,
            },
            adapter,
            context,
        );
    }

    let result = snapshot::run_with_context(
        adapter,
        &opts,
        &snapshot::SnapshotTarget {
            app_name: args.app.as_deref(),
            window_id: args.window_id.as_deref(),
        },
        context,
        args.timeout_ms
            .unwrap_or(snapshot::DEFAULT_SNAPSHOT_TIMEOUT_MS),
    )?;

    let diff = compare_with_baseline(&result, &opts, args.diff_from.as_deref(), context)?;
    let mut body = format_result(result)?;
    if let Some(diff) = diff {
        body["diff"] = diff;
    }
    Ok(body)
}

/// Keeps the fingerprints of this snapshot for a later `--diff`, and compares it with
/// `baseline` when one is named. Saving is best effort: a snapshot never fails for it.
fn compare_with_baseline(
    result: &snapshot::SnapshotResult,
    opts: &crate::adapter::TreeOptions,
    baseline: Option<&str>,
    context: &CommandContext,
) -> Result<Option<Value>, AppError> {
    let store = crate::refs::refs_store::RefStore::for_session(context.session_id())?;
    let window = crate::tree::tree_diff::DiffWindow {
        app: &result.window.app,
        window_id: &result.window.id,
        view: crate::tree::tree_diff::view_of(opts),
        complete: result.complete,
    };
    if let Some(snapshot_id) = result.snapshot_id.as_deref()
        && let Err(error) =
            crate::tree::tree_diff::save_fingerprints(&store, snapshot_id, &result.tree, &window)
    {
        tracing::debug!(?error, "snapshot: fingerprints not saved");
    }
    baseline
        .map(|baseline| {
            crate::tree::tree_diff::diff_against(&store, baseline, &result.tree, &window)
        })
        .transpose()
}

fn format_result(result: snapshot::SnapshotResult) -> Result<Value, AppError> {
    format_snapshot_fields(&result, None, None)
}

pub(crate) fn format_snapshot_fields(
    result: &snapshot::SnapshotResult,
    elapsed_ms: Option<u128>,
    matched_selector: Option<&str>,
) -> Result<Value, AppError> {
    let ref_count = result.refmap.len();
    let tree = serde_json::to_value(&result.tree)?;
    let win = &result.window;

    tracing::debug!(refs = ref_count, "tree: snapshot complete");

    let mut body = json!({
        "app": win.app,
        "ref_count": ref_count,
        "snapshot_id": result.snapshot_id,
        "complete": result.complete,
        "tree": tree
    });
    if !win.is_process_owner() {
        body["window"] = json!({ "id": win.id, "title": win.title });
    }
    if !result.complete {
        body["truncated"] = json!(true);
        body["nodes_observed"] = json!(result.nodes_observed);
    }
    if result.identity_unproven.is_some() {
        body["identity_unproven"] = json!(true);
    }
    if let Some(ms) = elapsed_ms {
        body["elapsed_ms"] = json!(ms);
    }
    if let Some(selector) = matched_selector {
        body["matched_selector"] = json!(selector);
    }
    Ok(body)
}

#[cfg(test)]
#[path = "snapshot_tests.rs"]
mod tests;
