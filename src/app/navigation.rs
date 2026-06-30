//! Scrolling and link navigation commands.

use crate::browser::open_link;

use super::App;
use super::scroll::HALF_PAGE_SCROLL_ANIM_SPEED;

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
        let max = self.max_scroll();
        self.view_state = self.view_state.clone().half_page_down(max);
    }

    pub(crate) fn half_page_up(&mut self) {
        self.scroll_anim_speed = HALF_PAGE_SCROLL_ANIM_SPEED;
        self.view_state = self.view_state.clone().half_page_up();
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
                            self.error_message =
                                Some(format!("failed to load preview: {}", link.url.as_str()));
                        }
                    } else if let Err(e) = open_link(&link.url) {
                        self.error_message = Some(e.to_string());
                    }
                } else {
                    self.error_message = Some(format!("dangling link {}", id));
                }
            }
            None => self.error_message = Some("no link selected".to_string()),
        }
    }

    pub(crate) fn close_preview(&mut self) {
        self.view_state = self.view_state.clone().close_preview();
    }

    pub(crate) fn scroll_to_selected_link(&mut self) {
        // Keep the selected link visible on screen. For now, rely on the user to scroll.
        // A future improvement would compute the Y position of each link occurrence.
    }
}
