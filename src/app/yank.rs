//! Yank helpers: link URL, heading slug, and code block.

use crate::clipboard::copy_to_clipboard;
use crate::domain::Block;
use crate::error::AppError;

use super::App;
use super::status::truncate_status;

impl App {
    pub(crate) fn yank_link_url(&mut self) -> Result<(), AppError> {
        let Some(id) = self.view_state.selected_link() else {
            self.set_status_message("no link selected — press n to select".into());
            return Ok(());
        };
        let Some(link) = self.document.links.get(id.0) else {
            self.set_status_error(format!("dangling link {id}"));
            return Ok(());
        };
        let url = link.url.as_str().to_string();
        copy_to_clipboard(&url)?;
        self.set_status_message(format!("yanked link ({})", truncate_status(&url, 40)));
        Ok(())
    }

    pub(crate) fn yank_heading_slug(&mut self) -> Result<(), AppError> {
        let Some(slug) = self.current_heading_slug() else {
            self.set_status_message("no heading at scroll position".into());
            return Ok(());
        };
        let text = format!("#{slug}");
        copy_to_clipboard(&text)?;
        self.set_status_message(format!("yanked heading {text}"));
        Ok(())
    }

    pub(crate) fn yank_code_block(&mut self) -> Result<(), AppError> {
        let Some(content) = self.code_block_in_viewport() else {
            self.set_status_message("no code block in view".into());
            return Ok(());
        };
        let len = content.chars().count();
        copy_to_clipboard(&content)?;
        self.set_status_message(format!("yanked code block ({len} characters)"));
        Ok(())
    }

    fn current_heading_slug(&mut self) -> Option<String> {
        let scroll = self.view_state.scroll().offset();
        let index = self.heading_index_at_scroll(scroll)?;
        self.heading_cache
            .entries()
            .get(index)
            .map(|entry| entry.slug.clone())
    }

    fn code_block_in_viewport(&self) -> Option<String> {
        let width = self.document_width();
        let ctx = self.render_context();
        let scroll = self.view_state.scroll().offset();
        let view_end = scroll + self.content_height() as usize;

        let mut first_after: Option<String> = None;
        for (top, block, height) in crate::render::block_tops(&self.document, width, &ctx) {
            let Block::CodeBlock(cb) = block else {
                continue;
            };
            if top + height > scroll && top < view_end {
                return Some(cb.content.clone());
            }
            if first_after.is_none() && top >= scroll {
                first_after = Some(cb.content.clone());
            }
        }
        first_after.or_else(|| {
            self.document.blocks.iter().find_map(|block| match block {
                Block::CodeBlock(cb) => Some(cb.content.clone()),
                _ => None,
            })
        })
    }
}
