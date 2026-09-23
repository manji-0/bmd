//! Scrolling and link navigation commands.

use crate::browser::open_link;
use crate::domain::{
    AnchorStackEmpty, AnchorStackFull, FixedScrollPrior, NavBackPlan, NavResetPlan,
    anchor_stack_limit_message, plan_back, plan_reset,
};
use crate::render::{find_heading_line_by_anchor, next_heading_line, prev_heading_line};

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
        self.scroll.anim_speed = HALF_PAGE_SCROLL_ANIM_SPEED;
        let half = self.content_height() as usize / 2;
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().scroll_down(half, max);
    }

    pub(crate) fn half_page_up(&mut self) {
        self.scroll.anim_speed = HALF_PAGE_SCROLL_ANIM_SPEED;
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
        let targets = self.document_nav_targets();
        self.view_state = self.view_state.clone().select_next_nav_in(&targets);
        self.scroll_to_selected_nav();
        self.maybe_warm_selected_preview();
    }

    pub(crate) fn prev_link(&mut self) {
        let targets = self.document_nav_targets();
        self.view_state = self.view_state.clone().select_prev_nav_in(&targets);
        self.scroll_to_selected_nav();
        self.maybe_warm_selected_preview();
    }

    fn document_nav_targets(&self) -> Vec<crate::domain::NavTarget> {
        let ctx = self.render_context();
        let width = self.document_width();
        crate::render::collect_nav_targets(&self.document, width, &ctx)
    }

    fn scroll_to_selected_nav(&mut self) {
        let Some(target) = self.view_state.selected_nav() else {
            return;
        };
        let ctx = self.render_context();
        let width = self.document_width();
        let Some(line) =
            crate::render::find_nav_target_line_offset(&self.document, width, &ctx, target)
        else {
            return;
        };
        self.scroll_to_line(line);
    }

    pub(crate) fn next_heading(&mut self) {
        self.refresh_heading_catalog();
        let scroll = self.view_state.scroll().offset();
        let line = {
            let headings = self.heading_cache.entries();
            next_heading_line(headings, scroll)
                .or_else(|| headings.last().map(|heading| heading.line_offset))
        };
        if let Some(line) = line {
            self.scroll_to_line(line);
        }
    }

    pub(crate) fn prev_heading(&mut self) {
        self.refresh_heading_catalog();
        let scroll = self.view_state.scroll().offset();
        let line = prev_heading_line(self.heading_cache.entries(), scroll);
        if let Some(line) = line {
            self.scroll_to_line(line);
        }
    }

    pub(crate) fn open_link_by_id(&mut self, id: crate::domain::LinkId) {
        self.view_state = self.view_state.clone().with_selected_link(id);
        self.open_current_link();
    }

    pub(crate) fn open_current_link(&mut self) {
        if let Some(footnote_id) = self.view_state.selected_footnote() {
            self.open_footnote(footnote_id);
            return;
        }
        let Some(id) = self.view_state.selected_link() else {
            self.set_status_message("no link selected — press n to select".into());
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
        if link.kind == crate::domain::LinkKind::Toc {
            self.preview.toc_selected = 0;
            self.open_preview_now(id);
            return;
        }
        if link.kind.is_preview() {
            let terminal_size = self.view_state.terminal_size();
            if self.picker.protocol_type() == ratatui_image::picker::ProtocolType::Halfblocks {
                let outcome = match link.kind {
                    crate::domain::LinkKind::Mermaid => crate::domain::mermaid_diagram_index(&url)
                        .and_then(|idx| self.document.mermaid_diagrams.get(idx))
                        .map(|diag| crate::render::open_mermaid_externally(&diag.source)),
                    crate::domain::LinkKind::Image => {
                        Some(crate::render::open_markdown_image_externally(
                            &url,
                            self.base_path.as_deref(),
                        ))
                    }
                    _ => None,
                };
                if let Some(Err(e)) = outcome {
                    self.set_status_message(e.to_string());
                }
            }
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
                self.preview.pending = None;
                self.open_preview_now(id);
            } else {
                self.preview.pending = Some(id);
                self.set_status_message(super::preview::preview_waiting_message(link.kind));
            }
        } else if link.kind == crate::domain::LinkKind::Web {
            if let Some(crate::github::GitHubUrl::Blob(blob)) =
                crate::github::parse_github_url(&url)
                && crate::github::is_supported_document_extension(&blob.path)
            {
                let fragment = crate::github::url_fragment(&url).map(str::to_string);
                self.open_github_blob_link(&blob, fragment.as_deref());
                return;
            }
            if let Err(e) = open_link(&link.url) {
                self.set_status_message(e.to_string());
            }
        }
    }

    pub(crate) fn close_preview(&mut self) {
        self.preview.pending = None;
        self.reset_preview_zoom();
        self.view_state = self.view_state.clone().close_preview();
    }

    /// Pop one scroll position from the anchor stack, or the previous document.
    ///
    /// Esc and `O` share this one-step back model. Status reports the destination
    /// (`back → file.md` / `back → previous position`).
    pub(crate) fn nav_back(&mut self) {
        match plan_back(&self.nav_stack, self.doc_stack.len_frames()) {
            NavBackPlan::AnchorStep => self.apply_anchor_back(),
            NavBackPlan::DocumentStep(idle) => self.doc_back(idle),
            NavBackPlan::Idle => self.set_status_message("nothing to go back to".into()),
        }
    }

    /// Reset the anchor stack, or return to the root document on the file stack.
    ///
    /// Not bound by default (Esc/`O` step back one layer). Remap `nav_reset` in
    /// config when a full drain is wanted. Anchor jumps reset before documents.
    pub(crate) fn nav_reset(&mut self) {
        match plan_reset(&self.nav_stack, self.doc_stack.len_frames()) {
            NavResetPlan::AnchorReset => self.apply_anchor_reset(),
            NavResetPlan::DocumentReset(idle) => self.doc_reset(idle),
            NavResetPlan::Idle => self.set_status_message("nothing to reset".into()),
        }
    }

    fn apply_anchor_back(&mut self) {
        match self.nav_stack.step_back() {
            Ok(offset) => {
                let max = self.max_scroll();
                self.view_state = self.view_state.clone().scroll_to(offset, max);
                self.snap_scroll_visual();
                self.set_status_message("back → previous position".into());
            }
            Err(AnchorStackEmpty) => {
                self.set_status_message("nothing to go back to".into());
            }
        }
    }

    fn apply_anchor_reset(&mut self) {
        let Ok(origin) = self.nav_stack.step_reset() else {
            self.set_status_message("nothing to reset".into());
            return;
        };
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().scroll_to(origin, max);
        self.snap_scroll_visual();
        self.set_status_message("reset → previous position".into());
    }

    pub(crate) fn follow_anchor(&mut self, anchor: &str) {
        let ctx = self.render_context();
        let width = self.document_width();
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

    pub(crate) fn jump_to_toc_heading(&mut self) {
        self.refresh_heading_catalog();
        let slug = self
            .heading_cache
            .entries()
            .get(self.preview.toc_selected)
            .map(|entry| entry.slug.clone());
        let Some(slug) = slug else {
            return;
        };
        self.close_preview();
        self.follow_anchor(&slug);
    }

    pub(crate) fn toc_select_next(&mut self) {
        self.refresh_heading_catalog();
        let count = self.heading_cache.entries().len();
        if count == 0 {
            return;
        }
        self.preview.toc_selected = (self.preview.toc_selected + 1) % count;
    }

    pub(crate) fn toc_select_prev(&mut self) {
        self.refresh_heading_catalog();
        let count = self.heading_cache.entries().len();
        if count == 0 {
            return;
        }
        self.preview.toc_selected = if self.preview.toc_selected == 0 {
            count - 1
        } else {
            self.preview.toc_selected - 1
        };
    }

    fn scroll_to_line(&mut self, line_offset: usize) {
        let max = self.max_scroll();
        let target = scroll_link_target(line_offset, max, &self.view_state);
        self.view_state = self.view_state.clone().scroll_to(target, max);
        self.snap_scroll_visual();
    }

    pub(crate) fn open_footnote(&mut self, footnote_id: crate::domain::FootnoteId) {
        if self.document.footnotes.get(footnote_id.0).is_none() {
            self.set_status_message(format!("footnote not found: {footnote_id}"));
            return;
        }
        self.reset_preview_zoom();
        self.view_state = self.view_state.clone().open_footnote_preview(footnote_id);
    }
}
