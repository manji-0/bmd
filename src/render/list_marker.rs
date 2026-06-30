//! List marker width and label helpers shared by layout, measure, and render.

use unicode_width::UnicodeWidthStr;

use crate::domain::{ChecklistState, List, ListItem};

pub(crate) fn list_marker_width_at(
    list: &List,
    item_idx: usize,
    item: &ListItem,
    checklist_state: &ChecklistState,
) -> usize {
    if item.checklist_id.is_some() {
        checklist_state.style().marker_width()
    } else if list.ordered {
        format!("{}.", item_idx + 1).width() + 1
    } else {
        2
    }
}

pub(crate) fn list_marker_label(
    list: &List,
    item_idx: usize,
    item: &ListItem,
    checklist_state: &ChecklistState,
) -> String {
    if item.checklist_id.is_some() {
        let style = checklist_state.style();
        if checklist_state.checked(item) {
            style.checked_marker().to_string()
        } else {
            style.unchecked_marker().to_string()
        }
    } else if list.ordered {
        format!("{}. ", item_idx + 1)
    } else {
        "• ".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Block, ChecklistId, ChecklistStyle, Inline};

    #[test]
    fn checklist_label_uses_unicode_glyphs_by_default() {
        let item = ListItem {
            checklist_id: Some(ChecklistId(0)),
            checked: false,
            content: vec![Block::Paragraph(vec![Inline::Text("task".into())])],
        };
        let list = List {
            ordered: false,
            items: vec![item.clone()],
        };
        let state = ChecklistState::new(ChecklistStyle::Unicode);
        assert_eq!(list_marker_label(&list, 0, &item, &state), "☐ ");
    }
}
