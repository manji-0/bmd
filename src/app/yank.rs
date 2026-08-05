//! Yank helpers: link URL, heading slug, and code block.

use crate::clipboard::copy_to_clipboard;
use crate::domain::Block;
use crate::error::AppError;
use crate::render::measure_block_height;

use super::App;

impl App {
    pub(crate) fn yank_link_url(&mut self) -> Result<(), AppError> {
        let Some(id) = self.view_state.selected_link() else {
            self.set_status_message("no link selected — press n to select".into());
            return Ok(());
        };
        let Some(link) = self.document.links.get(id.0) else {
            self.set_status_message(format!("dangling link {id}"));
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
        let headings = self.heading_offsets();
        let index = headings
            .iter()
            .rposition(|(offset, _)| *offset <= scroll)
            .or_else(|| headings.first().map(|_| 0))?;
        let entries = self.collect_toc_entries();
        entries.get(index).map(|(_, _, slug)| slug.clone())
    }

    fn code_block_in_viewport(&self) -> Option<String> {
        let width = self.document_width();
        let ctx = self.render_context();
        let scroll = self.view_state.scroll().offset();
        let view_end = scroll + self.content_height() as usize;

        let mut offset = 0usize;
        let mut first_after: Option<String> = None;
        for (idx, block) in self.document.blocks.iter().enumerate() {
            let gap = if idx == 0 { 0 } else { 1 };
            offset += gap;
            let height = measure_block_height(block, idx, width, &ctx);
            let end = offset + height;
            if let Block::CodeBlock(cb) = block {
                let intersects = end > scroll && offset < view_end;
                if intersects {
                    return Some(cb.content.clone());
                }
                if first_after.is_none() && offset >= scroll {
                    first_after = Some(cb.content.clone());
                }
            }
            offset = end;
        }
        first_after.or_else(|| {
            self.document.blocks.iter().find_map(|block| match block {
                Block::CodeBlock(cb) => Some(cb.content.clone()),
                _ => None,
            })
        })
    }
}

fn truncate_status(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_string();
    }
    let truncated: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}…")
}
