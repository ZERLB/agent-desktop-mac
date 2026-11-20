use crate::{
    AppError,
    actions::action::Action,
    adapter::PlatformAdapter,
    commands::combo::{ensure_combo_allowed, parse_combo_normalized},
    context::CommandContext,
};
use serde_json::Value;

pub struct PressArgs {
    pub combo: String,
    pub app: Option<String>,
    pub window_id: Option<String>,
    pub force: bool,
}

pub fn execute(
    args: PressArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
) -> Result<Value, AppError> {
    super::helpers::validate_post_action_wait(context)?;
    let combo = parse_combo_normalized(&args.combo)?;
    ensure_combo_allowed(&combo, &args.combo, args.force, adapter)?;
    let deadline = crate::Deadline::standard()?;

    let mut pressed_process = None;
    let mut kept_window = None;
    let target = target_app(&args, adapter, context, deadline)?;
    let result = if let Some((expected, window, kept)) = target {
        let lease = adapter.acquire_interaction_lease(deadline)?;
        let live = crate::commands::helpers::revalidate_app_for_mutation(
            adapter,
            &expected,
            lease.deadline(),
        )?;
        let process = crate::commands::helpers::process_identity(&live)?;
        pressed_process = Some(process.pid);
        kept_window = window
            .as_ref()
            .zip(kept)
            .and_then(|(window, kept)| kept_note(&kept, window));
        if context.physical_input_policy().is_headed() {
            match &window {
                Some(window) => crate::actions::headed_focus::focus_exact_window(
                    window, adapter, context, &lease,
                )?,
                None => crate::actions::headed_focus::focus_process_window(
                    process.clone(),
                    adapter,
                    context,
                    &lease,
                )?,
            };
        }
        if !context.physical_input_policy().is_headed()
            && let Some(window) = &window
        {
            adapter.press_key_for_window(window, &combo, context.physical_input_policy(), &lease)?
        } else {
            adapter.press_key_for_app(process, &combo, context.physical_input_policy(), &lease)?
        }
    } else {
        let lease = adapter.acquire_interaction_lease(deadline)?;
        let handle = crate::adapter::NativeHandle::null();
        adapter.execute_action(
            &handle,
            context.request_base(Action::PressKey(combo)),
            &lease,
        )?
    };
    crate::cursor_overlay::follow_caret_after_key(adapter, context, pressed_process);
    let mut response = serde_json::to_value(result)?;
    let waited_window = match (&kept_window, &mut response) {
        (Some(note), Value::Object(fields)) => {
            fields.insert("window_affinity".into(), note.clone());
            note["window_id"].as_str().map(str::to_owned)
        }
        _ => None,
    };
    super::helpers::apply_scoped_post_action_wait(
        response,
        args.app,
        args.window_id.or(waited_window),
        adapter,
        context,
    )
}

type PressTarget = (
    crate::AppInfo,
    Option<crate::WindowInfo>,
    Option<super::window_affinity::Affinity>,
);

/// The application and window a key goes to. A caller that names only an
/// application keeps the window it last snapshotted there, so its keys do not
/// follow the person into another window of the same application.
fn target_app(
    args: &PressArgs,
    adapter: &dyn PlatformAdapter,
    context: &CommandContext,
    deadline: crate::Deadline,
) -> Result<Option<PressTarget>, AppError> {
    if args.app.is_none() && args.window_id.is_none() {
        return Ok(None);
    }
    let (resolved, kept) = super::window_affinity::with_sticky_window(
        context,
        args.app.as_deref(),
        args.window_id.as_deref(),
        true,
        |window_id| resolve_target(args.app.as_deref(), window_id, adapter, deadline),
    )?;
    Ok(Some((resolved.0, resolved.1, kept)))
}

fn resolve_target(
    app: Option<&str>,
    window_id: Option<&str>,
    adapter: &dyn PlatformAdapter,
    deadline: crate::Deadline,
) -> Result<(crate::AppInfo, Option<crate::WindowInfo>), AppError> {
    let Some(window_id) = window_id else {
        let app = crate::commands::helpers::resolve_app(app, adapter, deadline)?;
        return Ok((app, None));
    };
    let window = crate::snapshot::resolve_window(adapter, app, Some(window_id), deadline)?;
    let app = crate::system::app_lookup::resolve_app_owning(&window, adapter, deadline)?;
    Ok((app, Some(window)))
}

fn kept_note(
    affinity: &super::window_affinity::Affinity,
    window: &crate::WindowInfo,
) -> Option<Value> {
    super::window_affinity::kept_note(affinity, window.state.is_focused)
}

#[cfg(test)]
#[path = "press_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "press_window_tests.rs"]
mod window_tests;
