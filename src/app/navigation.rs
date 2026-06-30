//! Scrolling and link navigation commands.

use crate::browser::open_link;
use crate::render::find_link_line_offset;

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
        self.view_state = self.view_state.clone().select_next_link(&self.document);
        self.scroll_to_selected_link();
    }

    pub(crate) fn prev_link(&mut self) {
        self.view_state = self.view_state.clone().select_prev_link(&self.document);
        self.scroll_to_selected_link();
    }

    pub(crate) fn open_current_link(&mut self) {
        match self.view_state.selected_link() {
            Some(id) => {
                if let Some(link) = self.document.links.get(id.0) {
                    if link.kind.is_preview() {
                        if self
                            .rendered
                            .preview_protocol(id.0, link.kind, link.url.as_str())
                            .is_some()
                        {
                            self.view_state = self.view_state.clone().open_preview(id);
                        } else {
                            self.set_status_message(format!(
                                "failed to load preview: {}",
                                link.url.as_str()
                            ));
                        }
                    } else if let Err(e) = open_link(&link.url) {
                        self.set_status_message(e.to_string());
                    }
                } else {
                    self.set_status_message(format!("dangling link {id}"));
                }
            }
            None => self.set_status_message("no link selected — press n to select a link".into()),
        }
    }

    pub(crate) fn close_preview(&mut self) {
        self.view_state = self.view_state.clone().close_preview();
    }

    pub(crate) fn scroll_to_selected_link(&mut self) {
        let Some(id) = self.view_state.selected_link() else {
            return;
        };
        let ctx = self.render_context();
        let width = self.view_state.terminal_size().width();
        let Some(line_offset) = find_link_line_offset(&self.document, width, &ctx, id) else {
            return;
        };
        let max = self.max_scroll();
        let target = scroll_link_target(line_offset, max, &self.view_state);
        self.view_state = self.view_state.clone().scroll_to(target);
    }
}
