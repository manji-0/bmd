//! Mermaid diagram rendering.

use std::collections::HashMap;

use merman::render::{HeadlessRenderer, raster::RasterOptions};
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    text::{Line, Text},
    widgets::{Paragraph, Widget},
};
use ratatui_image::protocol::Protocol;

use crate::domain::{Block, Document, MermaidDiagram};
use crate::error::AppError;

use super::context::RenderContext;
use super::image::{preload_markdown_images, terminal_image_protocol};

/// Cache of pre-rendered terminal images.
pub struct RenderedDocument {
    pub images: HashMap<usize, Protocol>,
    pub markdown_images: HashMap<String, Protocol>,
}

impl RenderedDocument {
    /// Render every `mermaid` block and markdown image to a terminal image protocol.
    ///
    /// # Errors
    ///
    /// Returns `AppError` if the terminal image protocol cannot be created. Individual
    /// render failures are logged and skipped (the widget renders a placeholder instead).
    pub fn new(
        document: &Document,
        picker: &ratatui_image::picker::Picker,
        width: u16,
        base_path: Option<&std::path::Path>,
    ) -> Result<Self, AppError> {
        let mut images = HashMap::new();
        for (idx, block) in document.blocks.iter().enumerate() {
            if let Block::Mermaid(diag) = block {
                match render_mermaid_image(diag, picker, width) {
                    Ok(protocol) => {
                        images.insert(idx, protocol);
                    }
                    Err(e) => {
                        eprintln!("[bmd] failed to render mermaid block {idx}: {e}");
                    }
                }
            }
        }
        let markdown_images = preload_markdown_images(document, picker, width, base_path);
        Ok(Self {
            images,
            markdown_images,
        })
    }
}

fn render_mermaid_image(
    diag: &MermaidDiagram,
    picker: &ratatui_image::picker::Picker,
    max_width: u16,
) -> Result<Protocol, AppError> {
    let renderer = HeadlessRenderer::new();
    let options = RasterOptions::default();
    let png = renderer
        .render_png_sync(&diag.source, &options)?
        .ok_or(AppError::MermaidNoDiagram)?;
    let dyn_img = image::load_from_memory(&png)?;

    terminal_image_protocol(
        dyn_img,
        picker,
        diag.estimated_width().min(max_width).max(20),
    )
}

pub(crate) fn measure_mermaid_height(ctx: &RenderContext, block_idx: usize, width: u16) -> usize {
    if let Some(protocol) = ctx.rendered.images.get(&block_idx) {
        return protocol.size().height as usize;
    }
    let cols = (width as usize).min(160);
    (cols * 9).div_ceil(16).max(6)
}

pub(crate) fn render_mermaid(
    _diag: &MermaidDiagram,
    block_idx: usize,
    area: Rect,
    buf: &mut Buffer,
    skip_rows: usize,
    ctx: &RenderContext,
) {
    if let Some(protocol) = ctx.rendered.images.get(&block_idx) {
        // If partially scrolled, render with clipping.
        let image = ratatui_image::Image::new(protocol).allow_clipping(true);
        // Shift the render area up by skip_rows by using a sub-rect.
        if skip_rows == 0 {
            image.render(area, buf);
        } else {
            // Clip from below is not directly supported; just render full image.
            image.render(area, buf);
        }
    } else {
        let placeholder = Paragraph::new(Text::from(vec![Line::styled(
            "[mermaid render failed or unsupported]",
            ctx.theme.mermaid_placeholder,
        )]));
        placeholder.render(area, buf);
    }
}
