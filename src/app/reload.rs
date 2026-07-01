//! Automatic reload when the watched file changes on disk.

use std::path::{Path, PathBuf};
use std::time::{Instant, SystemTime};

use crate::error::AppError;
use crate::parse::parse;
use crate::render::RenderedDocument;

use super::App;

pub(crate) const RELOAD_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// Tracks modification time for a file-backed document.
#[derive(Clone)]
pub(crate) struct FileWatch {
    path: PathBuf,
    last_modified: SystemTime,
}

impl FileWatch {
    pub(crate) fn new(path: PathBuf) -> Result<Self, AppError> {
        Ok(Self {
            last_modified: file_modified_time(&path)?,
            path,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    pub(crate) fn poll_changed(&mut self) -> Result<bool, AppError> {
        let modified = file_modified_time(&self.path)?;
        if modified > self.last_modified {
            self.last_modified = modified;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub(crate) fn refresh_timestamp(&mut self) -> Result<(), AppError> {
        self.last_modified = file_modified_time(&self.path)?;
        Ok(())
    }
}

fn file_modified_time(path: &Path) -> Result<SystemTime, AppError> {
    Ok(std::fs::metadata(path)?.modified()?)
}

impl App {
    pub(crate) fn poll_file_reload(&mut self, now: Instant) -> Result<bool, AppError> {
        if self.file_watch.is_none() {
            return Ok(false);
        }
        if now < self.next_reload_poll {
            return Ok(false);
        }
        self.next_reload_poll = now + RELOAD_POLL_INTERVAL;

        let Some(watch) = &mut self.file_watch else {
            return Ok(false);
        };
        if !watch.poll_changed()? {
            return Ok(false);
        }
        self.reload_from_disk()
    }

    /// Re-read the watched file, replace the document, and keep scroll offset only.
    pub(crate) fn reload_from_disk(&mut self) -> Result<bool, AppError> {
        let Some(watch) = &self.file_watch else {
            return Ok(false);
        };
        let path = watch.path().to_path_buf();
        let content = match std::fs::read_to_string(&path) {
            Ok(content) => content,
            Err(e) => {
                self.set_status_message(format!("reload failed: {e}"));
                return Ok(false);
            }
        };
        let document = match parse(&content) {
            Ok(document) => document,
            Err(e) => {
                self.set_status_message(format!("reload parse error: {e}"));
                if let Some(watch) = &mut self.file_watch {
                    let _ = watch.refresh_timestamp();
                }
                return Ok(false);
            }
        };

        let scroll_offset = self.view_state.scroll().offset();
        let terminal_size = self.view_state.terminal_size();
        self.document = document;
        self.rendered =
            RenderedDocument::new(&self.document, &self.picker, terminal_size, Some(&path))?;
        self.bump_document_revision();
        self.document_cache.invalidate();
        self.preview_render_cache.clear();
        self.pending_preview = None;
        self.checklist_state =
            crate::domain::ChecklistState::new(crate::domain::ChecklistStyle::from_env());
        self.help_visible = false;
        self.nav_stack.clear();

        let max_scroll = self.max_scroll();
        self.view_state = self
            .view_state
            .clone()
            .reset_for_reload(scroll_offset, max_scroll);
        let offset = self.view_state.scroll().offset();
        self.scroll_visual = offset as f32;
        self.tracked_scroll_position = self.scroll_visual;
        self.scroll_key_down_at = None;
        self.images_reenable_at = None;
        self.show_terminal_images = true;
        self.mermaid_render.begin_document();
        self.image_render.begin_document();
        self.document_prefetch.begin_document();
        self.invalidate_prefetch_viewport();
        self.maybe_prefetch_visible_links();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    use crate::domain::TerminalSize;

    use ratatui_image::picker::Picker;

    fn temp_markdown_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("bmd-{name}-{}.md", std::process::id()))
    }

    #[test]
    fn file_watch_detects_modification() {
        let path = temp_markdown_path("watch");
        let _ = std::fs::remove_file(&path);
        std::fs::write(&path, "# v1\n").unwrap();
        let mut watch = FileWatch::new(path.clone()).unwrap();
        assert!(!watch.poll_changed().unwrap());

        thread::sleep(Duration::from_millis(1100));
        std::fs::write(&path, "# v2\n").unwrap();
        assert!(watch.poll_changed().unwrap());
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reload_preserves_scroll_offset() {
        let path = temp_markdown_path("reload");
        let _ = std::fs::remove_file(&path);
        let mut body = String::from("# Title\n\n");
        for i in 0..100 {
            body.push_str(&format!("paragraph {i}\n\n"));
        }
        std::fs::write(&path, &body).unwrap();

        let document = parse(&std::fs::read_to_string(&path).unwrap()).unwrap();
        let size = TerminalSize::new(80, 24).unwrap();
        let mut app = App::new_with_terminal_size(
            document,
            Picker::halfblocks(),
            Some(path.clone()),
            Some("sample.md".into()),
            size,
            crate::config::Config::default(),
        )
        .unwrap();

        app.scroll_down(20);
        let before = app.view_state.scroll().offset();
        assert!(before > 0);

        body.push_str("updated\n");
        thread::sleep(Duration::from_millis(1100));
        std::fs::write(&path, &body).unwrap();
        app.file_watch.as_mut().unwrap().poll_changed().unwrap();
        assert!(app.reload_from_disk().unwrap());
        assert_eq!(app.view_state.scroll().offset(), before);
        assert!(!app.view_state.is_search_active());
        assert!(app.view_state.mode().is_normal());
        let _ = std::fs::remove_file(path);
    }
}
