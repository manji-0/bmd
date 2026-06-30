//! Scrolling and link navigation commands.

use crate::browser::open_link;
use crate::render::{
    collect_heading_offsets, collect_visible_links, find_heading_line_by_anchor, next_heading_line,
    prev_heading_line,
};

use super::App;
use super::scroll::HALF_PAGE_SCROLL_ANIM_SPEED;
use super::status::scroll_link_target;

impl App {
    pub(crate) fn scroll_down(&mut self, n: usize) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().scroll_down(n, max);
        self.snap_scroll_visual();
    }

    pub(crate) fn scroll_up(&mut self, n: usize) {
        self.view_state = self.view_state.clone().scroll_up(n);
        self.snap_scroll_visual();
    }

    pub(crate) fn half_page_down(&mut self) {
        self.scroll_anim_speed = HALF_PAGE_SCROLL_ANIM_SPEED;
        let half = self.content_height() as usize / 2;
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().scroll_down(half, max);
    }

    pub(crate) fn half_page_up(&mut self) {
        self.scroll_anim_speed = HALF_PAGE_SCROLL_ANIM_SPEED;
        let half = self.content_height() as usize / 2;
        self.view_state = self.view_state.clone().scroll_up(half);
    }

    pub(crate) fn jump_to_top(&mut self) {
        self.view_state = self.view_state.clone().jump_to_top();
        self.snap_scroll_visual();
    }

    pub(crate) fn jump_to_bottom(&mut self) {
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().jump_to_bottom(max);
        self.snap_scroll_visual();
    }

    pub(crate) fn next_link(&mut self) {
        let visible = self.visible_links();
        self.view_state = self.view_state.clone().select_next_link_in(&visible);
    }

    pub(crate) fn prev_link(&mut self) {
        let visible = self.visible_links();
        self.view_state = self.view_state.clone().select_prev_link_in(&visible);
    }

    fn visible_links(&self) -> Vec<crate::domain::LinkId> {
        let ctx = self.render_context();
        let width = self.view_state.terminal_size().width();
        let scroll = self.view_state.scroll().offset();
        let visible_lines = self.content_height() as usize;
        collect_visible_links(&self.document, width, &ctx, scroll, visible_lines)
    }

    pub(crate) fn next_heading(&mut self) {
        let ctx = self.render_context();
        let width = self.view_state.terminal_size().width();
        let headings = collect_heading_offsets(&self.document, width, &ctx);
        let scroll = self.view_state.scroll().offset();
        if let Some(line) = next_heading_line(&headings, scroll) {
            self.scroll_to_line(line);
        } else if let Some((line, _)) = headings.last() {
            self.scroll_to_line(*line);
        }
    }

    pub(crate) fn prev_heading(&mut self) {
        let ctx = self.render_context();
        let width = self.view_state.terminal_size().width();
        let headings = collect_heading_offsets(&self.document, width, &ctx);
        let scroll = self.view_state.scroll().offset();
        if let Some(line) = prev_heading_line(&headings, scroll) {
            self.scroll_to_line(line);
        }
    }

    pub(crate) fn open_current_link(&mut self) {
        let Some(id) = self.view_state.selected_link() else {
            self.set_status_message("no link selected — press n to select a link".into());
            return;
        };
        let Some(link) = self.document.links.get(id.0).cloned() else {
            self.set_status_message(format!("dangling link {id}"));
            return;
        };
        let url = link.url.as_str().to_string();
        if link.kind == crate::domain::LinkKind::Anchor {
            let anchor = url.strip_prefix('#').unwrap_or(&url);
            self.follow_anchor(anchor);
            return;
        }
        if link.kind.is_preview() {
            if self
                .rendered
                .preview_protocol(id.0, link.kind, &url)
                .is_some()
            {
                self.view_state = self.view_state.clone().open_preview(id);
            } else {
                self.set_status_message(format!("failed to load preview: {url}"));
            }
        } else if let Err(e) = open_link(&link.url) {
            self.set_status_message(e.to_string());
        }
    }

    pub(crate) fn close_preview(&mut self) {
        self.view_state = self.view_state.clone().close_preview();
    }

    /// Pop one scroll position from the navigation stack and scroll there.
    pub(crate) fn nav_back(&mut self) {
        let Some(offset) = self.nav_stack.pop() else {
            self.set_status_message("navigation stack empty".into());
            return;
        };
        self.view_state = self.view_state.clone().scroll_to(offset);
        self.snap_scroll_visual();
    }

    /// Jump to the bottom of the navigation stack and clear it.
    pub(crate) fn nav_reset(&mut self) {
        let Some(offset) = self.nav_stack.bottom() else {
            return;
        };
        self.nav_stack.clear();
        self.view_state = self.view_state.clone().scroll_to(offset);
        self.snap_scroll_visual();
    }

    fn follow_anchor(&mut self, anchor: &str) {
        let ctx = self.render_context();
        let width = self.view_state.terminal_size().width();
        let Some(line) = find_heading_line_by_anchor(&self.document, width, &ctx, anchor) else {
            self.set_status_message(format!("heading not found: #{anchor}"));
            return;
        };
        let current = self.view_state.scroll().offset();
        self.nav_stack.push(current);
        self.scroll_to_line(line);
    }

    fn scroll_to_line(&mut self, line_offset: usize) {
        let max = self.max_scroll();
        let target = scroll_link_target(line_offset, max, &self.view_state);
        self.view_state = self.view_state.clone().scroll_to(target);
        self.snap_scroll_visual();
    }
}
