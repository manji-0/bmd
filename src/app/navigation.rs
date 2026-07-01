//! Scrolling and link navigation commands.

use crate::browser::open_link;
use crate::domain::{
    AnchorIdle, AnchorStackEmpty, AnchorStackFull, FixedScrollPrior, NavBackPlan, NavResetPlan,
    anchor_stack_limit_message, plan_back, plan_document_back, plan_document_reset, plan_reset,
};
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
        if link.kind == crate::domain::LinkKind::Document {
            self.open_document_link(&url);
            return;
        }
        if link.kind.is_preview() {
            let terminal_size = self.view_state.terminal_size();
            match link.kind {
                crate::domain::LinkKind::Mermaid => {
                    self.mermaid_render.request(
                        id,
                        &self.document,
                        &self.rendered,
                        &self.picker,
                        terminal_size,
                    );
                }
                crate::domain::LinkKind::Image => {
                    self.image_render.request(
                        id,
                        &self.document,
                        &self.rendered,
                        self.base_path.as_ref(),
                        &self.picker,
                        terminal_size,
                    );
                }
                _ => {}
            }
            if self.preview_ready_to_open(id) {
                self.pending_preview = None;
                self.open_preview_now(id);
            } else {
                self.pending_preview = Some(id);
                self.set_status_message(super::preview::preview_waiting_message(link.kind));
            }
        } else if let Err(e) = open_link(&link.url) {
            self.set_status_message(e.to_string());
        }
    }

    pub(crate) fn close_preview(&mut self) {
        self.pending_preview = None;
        self.view_state = self.view_state.clone().close_preview();
    }

    /// Pop one scroll position from the anchor stack, or the previous document.
    pub(crate) fn nav_back(&mut self) {
        let document_depth = self.doc_stack.len_frames();
        match plan_back(&self.nav_stack, document_depth) {
            NavBackPlan::AnchorStep => self.apply_anchor_back(),
            NavBackPlan::DocumentStep => {
                let Some(idle) = AnchorIdle::from_stack(&self.nav_stack) else {
                    return;
                };
                let Some(()) = plan_document_back(idle, document_depth) else {
                    return;
                };
                self.doc_back(idle);
            }
            NavBackPlan::Idle => self.set_status_message("navigation stack empty".into()),
        }
    }

    /// Reset the anchor stack, or return to the root document on the file stack.
    ///
    /// Anchor jumps must be fully reset before the document stack is consulted.
    pub(crate) fn nav_reset(&mut self) {
        let document_depth = self.doc_stack.len_frames();
        match plan_reset(&self.nav_stack, document_depth) {
            NavResetPlan::AnchorReset => self.apply_anchor_reset(),
            NavResetPlan::DocumentReset => {
                let Some(idle) = AnchorIdle::from_stack(&self.nav_stack) else {
                    return;
                };
                let Some(()) = plan_document_reset(idle, document_depth) else {
                    return;
                };
                self.doc_reset(idle);
            }
            NavResetPlan::Idle => {}
        }
    }

    fn apply_anchor_back(&mut self) {
        match self.nav_stack.step_back() {
            Ok(offset) => {
                self.view_state = self.view_state.clone().scroll_to(offset);
                self.snap_scroll_visual();
            }
            Err(AnchorStackEmpty) => {
                self.set_status_message("navigation stack empty".into());
            }
        }
    }

    fn apply_anchor_reset(&mut self) {
        let Ok(origin) = self.nav_stack.step_reset() else {
            return;
        };
        self.view_state = self.view_state.clone().scroll_to(origin);
        self.snap_scroll_visual();
    }

    pub(crate) fn follow_anchor(&mut self, anchor: &str) {
        let ctx = self.render_context();
        let width = self.view_state.terminal_size().width();
        let Some(line) = find_heading_line_by_anchor(&self.document, width, &ctx, anchor) else {
            self.set_status_message(format!("heading not found: #{anchor}"));
            return;
        };
        let prior = FixedScrollPrior::fix(self.view_state.scroll().offset());
        if let Err(AnchorStackFull) = self.nav_stack.fix_prior_on_link_jump(prior) {
            self.set_status_message(anchor_stack_limit_message());
            return;
        }
        self.scroll_to_line(line);
    }

    fn scroll_to_line(&mut self, line_offset: usize) {
        let max = self.max_scroll();
        let target = scroll_link_target(line_offset, max, &self.view_state);
        self.view_state = self.view_state.clone().scroll_to(target);
        self.snap_scroll_visual();
    }
}
