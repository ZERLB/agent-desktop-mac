//! The `AXOpen` fallback of a click.
//!
//! A Finder icon publishes `AXOpen` as its only action, and opening a file-system item
//! opens the document, launches the app or runs the script: that is a double-click,
//! not a click. An element that names a `file:` URL is therefore skipped here, so the
//! chain goes on to select it inside its container. Sidebar cells, which navigate
//! through `AXOpen` and name no URL, keep it.

#[cfg(target_os = "macos")]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};
    use core_foundation::base::{CFType, TCFType};
    use core_foundation::string::CFString;
    use core_foundation::url::CFURL;

    use crate::actions::chain::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;

    pub(crate) fn open_unless_file_item(
        element: &AXElement,
        deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        crate::tree::properties::attributes::set_messaging_timeout(element, deadline)?;
        if names_file_url(element, deadline) {
            return Ok(DeliveryOutcome::NotDelivered);
        }
        crate::actions::ax_helpers::perform_observed_action(element, "AXOpen", deadline)
    }

    fn names_file_url(element: &AXElement, deadline: Deadline) -> bool {
        let attribute = CFString::new("AXURL");
        let (error, value) = crate::tree::properties::ax_ipc::copy_attribute_value(
            element,
            attribute.as_concrete_TypeRef(),
            deadline,
        );
        if value.is_null() {
            return false;
        }
        let value = unsafe { CFType::wrap_under_create_rule(value) };
        error == accessibility_sys::kAXErrorSuccess
            && value
                .downcast::<CFURL>()
                .is_some_and(|url| super::is_file_url(&url.get_string().to_string()))
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use agent_desktop_core::{AdapterError, Deadline};

    use crate::actions::chain::chain_delivery::DeliveryOutcome;
    use crate::tree::AXElement;

    pub(crate) fn open_unless_file_item(
        _element: &AXElement,
        _deadline: Deadline,
    ) -> Result<DeliveryOutcome, AdapterError> {
        Ok(DeliveryOutcome::NotDelivered)
    }
}

pub(crate) use imp::open_unless_file_item;

fn is_file_url(url: &str) -> bool {
    url.get(..5)
        .is_some_and(|scheme| scheme.eq_ignore_ascii_case("file:"))
}

#[cfg(test)]
mod tests {
    use super::is_file_url;

    #[test]
    fn only_a_file_url_marks_a_file_system_item() {
        assert!(is_file_url("file:///.file/id=6571367.92294118"));
        assert!(is_file_url("FILE:///Users/someone/Beta.txt"));
        assert!(!is_file_url("https://example.com/file:///x"));
        assert!(!is_file_url("fil"));
    }
}
