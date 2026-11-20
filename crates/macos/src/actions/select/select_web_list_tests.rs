use super::is_list_role;

#[test]
fn a_page_drawn_list_is_a_list_menu_outline_or_table() {
    for role in ["AXList", "AXMenu", "AXOutline", "AXTable"] {
        assert!(is_list_role(role), "{role} holds options");
    }
    for role in ["AXGroup", "AXStaticText", "AXComboBox", "AXWebArea"] {
        assert!(!is_list_role(role), "{role} is not a list of options");
    }
}
