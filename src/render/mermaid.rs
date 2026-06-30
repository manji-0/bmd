//! Mermaid diagram rendering.

use std::collections::HashMap;

use merman::render::{HeadlessRenderer, raster::RasterOptions};
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    text::{Line, Text},
    widgets::{Paragraph, Widget},
};
use ratatui_image::{Resize, protocol::Protocol};

use crate::domain::{Block, Document, MermaidDiagram};
use crate::error::AppError;

use super::context::RenderContext;

/// Cache of pre-rendered mermaid images keyed by block index.
pub struct RenderedDocument {
    pub images: HashMap<usize, Protocol>,
}

impl RenderedDocument {
    /// Render every `mermaid` block to a terminal image protocol.
    ///
    /// # Errors
    ///
    /// Returns `AppError` if the terminal image protocol cannot be created. Individual
    /// mermaid failures are logged and skipped (the widget renders a placeholder instead).
    pub fn new(
        document: &Document,
        picker: &ratatui_image::picker::Picker,
        width: u16,
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
        Ok(Self { images })
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

    let font_size = picker.font_size();
    // Use a width proportional to the diagram's natural size so nodes and
    // text are not shrunk to an unreadable mosaic.
    let cols = diag.estimated_width().min(max_width).max(20) as u32;
    let rows = (dyn_img.height() as u32)
        .saturating_mul(cols)
        .saturating_mul(font_size.width as u32)
        .div_ceil(dyn_img.width().max(1))
        .div_ceil(font_size.height.max(1) as u32)
        .max(1);
    let size = Size::new(cols as u16, rows as u16);
    picker
        .new_protocol(dyn_img, size, Resize::Fit(None))
        .map_err(|e| AppError::TerminalImage(e.to_string()))
}

pub(crate) fn measure_mermaid_height(ctx: &RenderContext, width: u16) -> usize {
    // The rendered protocol has a fixed cell size; if it's cached, use it.
    if let Some((_idx, protocol)) = ctx.rendered.images.iter().next() {
        return protocol.size().height as usize;
    }
    // Fallback: approximate 16:9 height for a mid-size diagram.
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
