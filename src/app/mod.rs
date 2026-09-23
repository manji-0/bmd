//! Application loop and state.

mod checklist;
mod doc_stack;
mod document;
mod document_prefetch;
mod draw;
mod image_render;
mod input;
mod layout;
mod marks;
mod mermaid_render;
mod navigation;
mod outline;
mod pending;
mod preview;
mod reload;
mod scroll;
mod search;
pub mod status;
mod worker_pool;
mod yank;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event;

use ratatui::{Terminal, backend::Backend};
use ratatui_image::picker::Picker;

use crate::config::Config;
use crate::domain::{
    ChecklistState, ChecklistStyle, Document, Marks, NavStack, TerminalSize, TextSelection,
    ViewState,
};
use crate::error::AppError;
use crate::keymap::Keymap;
use crate::render::{
    DocumentRenderCache, HeadingCatalogCache, RenderContext, RenderedDocument, SyntaxAssets, Theme,
};

use doc_stack::DocStack;
use document_prefetch::DocumentPrefetchPool;
use image_render::ImageRenderPool;
use layout::terminal_size;
use mermaid_render::MermaidRenderPool;
use outline::OutlineUi;
use pending::PendingInput;
use preview::PreviewUi;
use reload::FileWatch;
use scroll::{ACTIVE_FRAME_INTERVAL, IDLE_POLL_INTERVAL, STATUS_MESSAGE_DURATION, ScrollUi};
use worker_pool::WorkerPool;

#[derive(Clone, PartialEq, Eq)]
struct PrefetchViewportKey {
    scroll: usize,
    visible_lines: usize,
    width: u16,
    document_path: Option<std::path::PathBuf>,
}

pub struct App {
    document: Document,
    rendered: RenderedDocument,
    view_state: ViewState,
    document_cache: DocumentRenderCache,
    scroll: ScrollUi,
    last_tick: Instant,
    syntax_assets: SyntaxAssets,
    theme: Theme,
    keymap: Keymap,
    checklist_state: ChecklistState,
    base_path: Option<std::path::PathBuf>,
    source_label: Option<String>,
    help_visible: bool,
    status_message: Option<String>,
    status_message_until: Option<Instant>,
    /// Live match count while typing `/`/`?` (`None` when not applicable).
    live_search_match_count: Option<usize>,
    picker: Picker,
    pub(crate) file_watch: Option<FileWatch>,
    next_reload_poll: Instant,
    nav_stack: NavStack,
    doc_stack: DocStack,
    mermaid_render: MermaidRenderPool,
    image_render: ImageRenderPool,
    document_prefetch: DocumentPrefetchPool,
    preview: PreviewUi,
    outline: OutlineUi,
    /// Vim-style scroll marks for the current document.
    marks: Marks,
    /// Two-key operator waiting for a second keystroke.
    pending_input: PendingInput,
    /// Mouse-selected text range in document logical coordinates.
    text_selection: Option<TextSelection>,
    /// In-progress mouse drag for text selection.
    selection_drag: Option<checklist::SelectionDrag>,
    last_prefetch_viewport: Option<PrefetchViewportKey>,
    document_revision: u64,
    heading_cache: HeadingCatalogCache,
    pub(crate) github_auth: Option<crate::github::GitHubAuth>,
    should_quit: bool,
    #[cfg(test)]
    pub(crate) fail_apply_document: bool,
    #[cfg(test)]
    pub(crate) fail_document_restore: bool,
}

#[cfg(test)]
impl App {
    pub(crate) fn document_cache_total_height(&self) -> usize {
        self.document_cache.total_height()
    }

    pub(crate) fn prefetched_document_ready(&self, path: &std::path::Path) -> bool {
        self.document_prefetch.ready_document(path).is_some()
    }
}

impl App {
    pub fn new(
        document: Document,
        picker: Picker,
        base_path: Option<std::path::PathBuf>,
        source_label: Option<String>,
    ) -> Result<Self, AppError> {
        Self::new_with_config(document, picker, base_path, source_label, Config::load()?)
    }

    pub fn new_with_config(
        document: Document,
        picker: Picker,
        base_path: Option<std::path::PathBuf>,
        source_label: Option<String>,
        config: Config,
    ) -> Result<Self, AppError> {
        Self::new_with_terminal_size(
            document,
            picker,
            base_path,
            source_label,
            terminal_size()?,
            config,
        )
    }

    pub fn new_with_terminal_size(
        document: Document,
        picker: Picker,
        base_path: Option<std::path::PathBuf>,
        source_label: Option<String>,
        terminal_size: TerminalSize,
        config: Config,
    ) -> Result<Self, AppError> {
        let rendered =
            RenderedDocument::new(&document, &picker, terminal_size, base_path.as_deref())?;
        let view_state = ViewState::new(terminal_size);
        let scroll_visual = view_state.scroll().offset() as f32;
        let now = Instant::now();
        let file_watch = base_path
            .as_ref()
            .and_then(|path| FileWatch::new(path.clone()).ok());
        let worker_pool = WorkerPool::shared();
        let mut app = Self {
            document,
            rendered,
            view_state,
            document_cache: DocumentRenderCache::default(),
            scroll: ScrollUi::new(scroll_visual, now),
            last_tick: now,
            syntax_assets: SyntaxAssets::new(),
            theme: config.theme,
            keymap: config.keymap,
            checklist_state: ChecklistState::new(ChecklistStyle::from_env()),
            base_path: base_path.clone(),
            source_label,
            help_visible: false,
            status_message: None,
            status_message_until: None,
            live_search_match_count: None,
            picker,
            file_watch,
            next_reload_poll: now,
            nav_stack: NavStack::default(),
            doc_stack: DocStack::default(),
            mermaid_render: MermaidRenderPool::new(Arc::clone(&worker_pool)),
            image_render: ImageRenderPool::new(Arc::clone(&worker_pool)),
            document_prefetch: DocumentPrefetchPool::new(worker_pool),
            preview: PreviewUi::default(),
            outline: OutlineUi::default(),
            marks: Marks::new(),
            pending_input: PendingInput::None,
            text_selection: None,
            selection_drag: None,
            last_prefetch_viewport: None,
            document_revision: 0,
            heading_cache: HeadingCatalogCache::default(),
            github_auth: None,
            should_quit: false,
            #[cfg(test)]
            fail_apply_document: false,
            #[cfg(test)]
            fail_document_restore: false,
        };
        app.maybe_prefetch_visible_links();
        Ok(app)
    }

    pub fn set_github_auth(&mut self, auth: Option<crate::github::GitHubAuth>) {
        self.github_auth = auth;
    }

    fn ensure_github_auth(&mut self) -> crate::github::GitHubAuth {
        if self.github_auth.is_none() {
            self.github_auth = Some(crate::github::resolve_auth());
        }
        self.github_auth.clone().unwrap()
    }

    pub(crate) fn prefetch_visible_links(&mut self) {
        let visible = self.visible_link_ids();
        let terminal = self.view_state.terminal_size();
        self.mermaid_render.prefetch_visible(
            &visible,
            &self.document,
            &self.rendered,
            &self.picker,
            terminal,
        );
        self.image_render.prefetch_visible(
            &visible,
            &self.document,
            &self.rendered,
            self.base_path.as_ref(),
            &self.picker,
            terminal,
        );
        self.document_prefetch
            .prefetch_visible(&visible, &self.document, self.base_path.as_ref());
    }

    pub(crate) fn maybe_prefetch_visible_links(&mut self) {
        let key = self.current_prefetch_viewport();
        if self.last_prefetch_viewport.as_ref() == Some(&key) {
            return;
        }
        self.last_prefetch_viewport = Some(key);
        self.prefetch_visible_links();
    }

    pub(crate) fn invalidate_prefetch_viewport(&mut self) {
        self.last_prefetch_viewport = None;
    }

    fn current_prefetch_viewport(&self) -> PrefetchViewportKey {
        PrefetchViewportKey {
            scroll: self.view_state.scroll().offset(),
            visible_lines: self.content_height() as usize,
            width: self.document_width(),
            document_path: self.base_path.clone(),
        }
    }

    pub(crate) fn visible_link_ids(&self) -> Vec<crate::domain::LinkId> {
        let ctx = self.render_context();
        let width = self.document_width();
        let scroll = self.view_state.scroll().offset();
        let visible_lines = self.content_height() as usize;
        crate::render::collect_visible_links(&self.document, width, &ctx, scroll, visible_lines)
    }

    pub(crate) fn poll_preview_renders(&mut self) -> bool {
        let terminal = self.view_state.terminal_size();
        let mermaid_dirty =
            self.mermaid_render
                .poll(&mut self.rendered, &self.document, &self.picker, terminal);
        let image_dirty = self.image_render.poll(
            &mut self.rendered,
            &self.document,
            self.base_path.as_ref(),
            &self.picker,
            terminal,
        );
        let document_prefetch_dirty = self.document_prefetch.poll();
        let pending_opened = self.try_complete_pending_preview();
        if mermaid_dirty || image_dirty {
            self.maybe_warm_selected_preview();
        }
        mermaid_dirty || image_dirty || document_prefetch_dirty || pending_opened
    }

    pub(crate) fn bump_document_revision(&mut self) {
        self.document_revision = self.document_revision.wrapping_add(1);
    }

    pub(crate) fn refresh_heading_catalog(&mut self) {
        let document_revision = self.document_revision;
        let width = self.document_width();
        let checklist_revision = self.checklist_state.revision();
        let ctx = RenderContext::new(
            &self.theme,
            &self.syntax_assets,
            &self.rendered,
            &self.document.links,
            &self.view_state,
            &self.checklist_state,
        );
        self.heading_cache.refresh(
            document_revision,
            width,
            checklist_revision,
            &self.document,
            &ctx,
        );
    }

    /// Content width available for document wrapping (excludes outline sidebar).
    pub(crate) fn document_width(&self) -> u16 {
        let full = self.view_state.terminal_size().width();
        let reserve = outline::outline_reserve_width(full, self.outline.visible);
        full.saturating_sub(reserve).max(1)
    }

    pub(crate) fn preview_work_pending(&self) -> bool {
        self.preview.pending.is_some()
            || self.mermaid_render.has_pending()
            || self.image_render.has_pending()
            || self.document_prefetch.has_pending()
    }

    pub(crate) fn set_status_message(&mut self, msg: String) {
        self.status_message = Some(msg);
        self.status_message_until = Some(Instant::now() + STATUS_MESSAGE_DURATION);
    }

    pub(crate) fn tick_status_message(&mut self, now: Instant) {
        if let Some(until) = self.status_message_until
            && now >= until
        {
            self.status_message = None;
            self.status_message_until = None;
        }
    }

    pub(crate) fn content_height(&self) -> u16 {
        layout::content_height(
            self.view_state.terminal_size().height(),
            self.view_state.mode(),
        )
    }

    pub fn run<B: Backend>(mut self, terminal: &mut Terminal<B>) -> Result<(), AppError>
    where
        AppError: From<B::Error>,
    {
        self.last_tick = Instant::now();
        let mut last_draw = Instant::now();
        self.draw_frame(terminal)?;

        while !self.should_quit {
            let now = Instant::now();
            let dt = now.saturating_duration_since(self.last_tick);
            self.last_tick = now;
            let mut dirty = false;

            while event::poll(Duration::ZERO)? {
                if self.handle_crossterm_event(event::read()?)? {
                    dirty = true;
                }
                if self.should_quit {
                    break;
                }
            }

            if self.should_quit {
                break;
            }

            if self.poll_terminal_resize()? {
                dirty = true;
            }

            if self.poll_file_reload(now)? {
                dirty = true;
            }

            let animating = self.tick_scroll_animation(dt);
            self.maybe_prefetch_visible_links();
            let mermaid_dirty = self.poll_preview_renders();
            let awaiting_preview = self.preview_work_pending();
            self.tick_status_message(now);

            if dirty || animating || mermaid_dirty {
                self.draw_frame(terminal)?;
                last_draw = now;
            }

            let frame_budget = if animating || awaiting_preview {
                ACTIVE_FRAME_INTERVAL
            } else {
                IDLE_POLL_INTERVAL
            };
            let wait = frame_budget.saturating_sub(last_draw.elapsed());
            if event::poll(wait)? {
                continue;
            }
            if animating || awaiting_preview {
                continue;
            }
        }
        Ok(())
    }
}
