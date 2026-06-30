//! Mermaid diagram rendering for the floating preview overlay.

use std::collections::HashMap;

use merman::render::{HeadlessRenderer, LayoutOptions, raster::RasterOptions};
use ratatui_image::protocol::Protocol;

use crate::domain::{Document, LinkKind, MermaidDiagram, TerminalSize};
use crate::error::AppError;

use super::image::{preload_markdown_images, preview_content_size, terminal_image_protocol};

/// Cache of pre-rendered terminal images for floating previews.
pub struct RenderedDocument {
    pub mermaid_images: HashMap<usize, Protocol>,
    pub markdown_images: HashMap<String, Protocol>,
}

impl RenderedDocument {
    /// Pre-render mermaid diagrams and markdown images for floating preview.
    ///
    /// # Errors
    ///
    /// Returns `AppError` if the terminal image protocol cannot be created. Individual
    /// render failures are logged and skipped.
    pub fn new(
        document: &Document,
        picker: &ratatui_image::picker::Picker,
        terminal: TerminalSize,
        base_path: Option<&std::path::Path>,
    ) -> Result<Self, AppError> {
        let mut mermaid_images = HashMap::new();
        for (link_id, link) in document.links.iter().enumerate() {
            if link.kind != LinkKind::Mermaid {
                continue;
            }
            let Some(diagram_idx) = mermaid_diagram_index(link.url.as_str()) else {
                continue;
            };
            let Some(diag) = document.mermaid_diagrams.get(diagram_idx) else {
                continue;
            };
            match render_mermaid_image(diag, picker, terminal) {
                Ok(protocol) => {
                    mermaid_images.insert(link_id, protocol);
                }
                Err(e) => {
                    eprintln!("[bmd] failed to render mermaid link {link_id}: {e}");
                }
            }
        }
        let markdown_images = preload_markdown_images(document, picker, terminal, base_path);
        Ok(Self {
            mermaid_images,
            markdown_images,
        })
    }

    pub(crate) fn preview_protocol(
        &self,
        link_id: usize,
        kind: LinkKind,
        url: &str,
    ) -> Option<&Protocol> {
        match kind {
            LinkKind::Image => self.markdown_images.get(url),
            LinkKind::Mermaid => self.mermaid_images.get(&link_id),
            LinkKind::Web => None,
        }
    }
}

pub(crate) fn mermaid_diagram_index(url: &str) -> Option<usize> {
    url.strip_prefix("bmd:mermaid:")?.parse().ok()
}

/// Supersample factor for mermaid PNG rasterization before terminal downscale.
const MERMAID_RASTER_SCALE: f32 = 3.0;

fn render_mermaid_image(
    diag: &MermaidDiagram,
    picker: &ratatui_image::picker::Picker,
    terminal: TerminalSize,
) -> Result<Protocol, AppError> {
    let target = preview_content_size(terminal);
    let font = picker.font_size();
    let mut layout = LayoutOptions::headless_svg_defaults();
    layout.viewport_width = target.width as f64 * font.width as f64;
    layout.viewport_height = target.height as f64 * font.height as f64;

    let renderer = HeadlessRenderer::new()
        .with_layout_options(layout)
        .with_diagram_id("bmd-mermaid");
    let options = RasterOptions {
        scale: MERMAID_RASTER_SCALE,
        ..Default::default()
    };
    let png = renderer
        .render_png_sync(&diag.source, &options)?
        .ok_or(AppError::MermaidNoDiagram)?;
    let dyn_img = image::load_from_memory(&png)?;

    terminal_image_protocol(dyn_img, picker, target)
}

#[cfg(test)]
mod tests {
    use super::mermaid_diagram_index;

    #[test]
    fn mermaid_diagram_index_parses_bmd_url() {
        assert_eq!(mermaid_diagram_index("bmd:mermaid:0"), Some(0));
        assert_eq!(mermaid_diagram_index("https://x"), None);
    }
}
