use agent_desktop_core::{AdapterError, Deadline};
use std::time::Instant;

use super::select_search::SelectSearch;
use crate::tree::AXElement;

const NEARBY_ANCESTORS: usize = 3;
const MAX_LIST_SEARCH_NODES: usize = 400;
const MAX_LIST_SEARCH_DEPTH: usize = 6;
const LIST_ROLES: &[&str] = &["AXList", "AXMenu", "AXOutline", "AXTable"];

/// Finds the option named `value` in the list a web combobox opened. A page
/// draws that list itself, so it is no menu of the application: it is the
/// list the control names (`aria-controls`), or a list near the control.
pub(super) fn open_list_item(
    control: &AXElement,
    value: &str,
    deadline: Deadline,
) -> Result<Option<AXElement>, AdapterError> {
    let instant = crate::tree::locator_deadline::from_operation(deadline)?;
    let linked = crate::tree::surface_read::elements(control, "AXLinkedUIElements", instant)
        .unwrap_or_default();
    if let Some(item) = linked
        .iter()
        .find_map(|list| named_item(list, value, deadline))
    {
        return Ok(Some(item));
    }
    let mut ancestor = parent(control, instant);
    for _ in 0..NEARBY_ANCESTORS {
        let Some(current) = ancestor else {
            break;
        };
        let found = lists_under(&current, instant)
            .iter()
            .find_map(|list| named_item(list, value, deadline));
        if found.is_some() {
            return Ok(found);
        }
        ancestor = parent(&current, instant);
    }
    Ok(None)
}

fn named_item(list: &AXElement, value: &str, deadline: Deadline) -> Option<AXElement> {
    super::select_menu::find_named_descendant(list, value, deadline, SelectSearch::MenuItem)
        .ok()
        .flatten()
}

fn lists_under(root: &AXElement, instant: Instant) -> Vec<AXElement> {
    let mut lists = Vec::new();
    let mut level = vec![root.clone()];
    let mut seen = 0_usize;
    for _ in 0..MAX_LIST_SEARCH_DEPTH {
        let mut next = Vec::new();
        for element in &level {
            seen += 1;
            if seen > MAX_LIST_SEARCH_NODES {
                return lists;
            }
            let role = crate::tree::surface_read::string(element, "AXRole", instant)
                .ok()
                .flatten();
            if role.as_deref().is_some_and(is_list_role) {
                lists.push(element.clone());
            } else {
                next.extend(
                    crate::tree::surface_read::elements(element, "AXChildren", instant)
                        .unwrap_or_default(),
                );
            }
        }
        if next.is_empty() {
            break;
        }
        level = next;
    }
    lists
}

fn is_list_role(role: &str) -> bool {
    LIST_ROLES.contains(&role)
}

fn parent(element: &AXElement, instant: Instant) -> Option<AXElement> {
    crate::tree::surface_read::element(element, "AXParent", instant)
        .ok()
        .flatten()
}

#[cfg(test)]
#[path = "select_web_list_tests.rs"]
mod tests;
