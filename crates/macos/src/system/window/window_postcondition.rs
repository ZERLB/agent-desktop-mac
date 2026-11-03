use agent_desktop_core::{AdapterError, Deadline, DeliverySemantics, Rect};

pub(crate) const TOLERANCE: f64 = 2.0;

pub(crate) fn wait_for_geometry(
    element: &crate::tree::AXElement,
    expected: Rect,
    position: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        let bounds = crate::tree::element_bounds::read_bounds_with_deadline(
            element,
            deadline_instant(deadline)?,
        )?;
        Ok(bounds.is_some_and(|actual| geometry_matches(actual, expected, position)))
    })
}

/// Waits until `AXMinimized` reads `expected`. While the minimize or restore animation
/// runs, the window answers `kAXErrorCannotComplete`; that read means "not yet", so it is
/// retried until the deadline instead of failing an operation that already took effect.
pub(crate) fn wait_for_minimized(
    element: &crate::tree::AXElement,
    expected: bool,
    deadline: Deadline,
) -> Result<(), AdapterError> {
    wait_until(deadline, || {
        match crate::tree::surface_read::boolean(
            element,
            "AXMinimized",
            deadline_instant(deadline)?,
        ) {
            Ok(value) => Ok(value == Some(expected)),
            Err(error) if is_retryable_read(&error) => Ok(false),
            Err(error) => Err(error),
        }
    })
}

fn is_retryable_read(error: &AdapterError) -> bool {
    error
        .details
        .as_ref()
        .and_then(|details| details.get("retryable"))
        .and_then(serde_json::Value::as_bool)
        == Some(true)
}

fn wait_until(
    deadline: Deadline,
    mut complete: impl FnMut() -> Result<bool, AdapterError>,
) -> Result<(), AdapterError> {
    loop {
        if complete().map_err(after_delivery)? {
            return Ok(());
        }
        if deadline.is_expired() {
            return Err(after_delivery(AdapterError::timeout(
                "Window operation did not reach its requested postcondition",
            )));
        }
        let pause = deadline
            .remaining_slice(std::time::Duration::from_millis(10))
            .map_err(after_delivery)?;
        std::thread::sleep(pause.min(std::time::Duration::from_millis(10)));
    }
}

fn geometry_matches(actual: Rect, expected: Rect, position: bool) -> bool {
    let values = if position {
        [(actual.x, expected.x), (actual.y, expected.y)]
    } else {
        [
            (actual.width, expected.width),
            (actual.height, expected.height),
        ]
    };
    values
        .into_iter()
        .all(|(actual, expected)| (actual - expected).abs() <= TOLERANCE)
}

fn deadline_instant(deadline: Deadline) -> Result<std::time::Instant, AdapterError> {
    std::time::Instant::now()
        .checked_add(deadline.remaining())
        .ok_or_else(|| AdapterError::internal("Window operation deadline is out of range"))
}

fn after_delivery(error: AdapterError) -> AdapterError {
    error.with_disposition(DeliverySemantics::delivered_unverified())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn geometry_verification_tolerates_ax_rounding_only() {
        let expected = Rect {
            x: 10.0,
            y: 20.0,
            width: 100.0,
            height: 80.0,
        };
        let rounded = Rect {
            x: 11.0,
            y: 19.0,
            width: 99.0,
            height: 81.0,
        };
        let wrong = Rect {
            width: 90.0,
            ..rounded
        };

        assert!(geometry_matches(rounded, expected, true));
        assert!(geometry_matches(rounded, expected, false));
        assert!(!geometry_matches(wrong, expected, false));
    }

    #[test]
    fn a_busy_window_read_is_retried_but_other_failures_are_not() {
        let busy = crate::tree::surface_read::map_error(
            "AXMinimized",
            accessibility_sys::kAXErrorCannotComplete,
        );
        let gone = crate::tree::surface_read::map_error(
            "AXMinimized",
            accessibility_sys::kAXErrorInvalidUIElement,
        );
        let denied = crate::tree::surface_read::map_error(
            "AXMinimized",
            accessibility_sys::kAXErrorAPIDisabled,
        );

        assert!(is_retryable_read(&busy));
        assert!(!is_retryable_read(&gone));
        assert!(!is_retryable_read(&denied));
        assert!(!is_retryable_read(&AdapterError::timeout("deadline")));
    }
}
