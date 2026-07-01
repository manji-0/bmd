//! Mermaid diagram rendering for the floating preview overlay.

use std::collections::HashMap;

use merman::render::{HeadlessRenderer, LayoutOptions, raster::RasterOptions};
use ratatui_image::protocol::Protocol;

use crate::domain::{Document, LinkKind, MermaidDiagram, TerminalSize};
use crate::error::AppError;

use super::image::{preview_content_size, terminal_image_protocol};

/// Cache of pre-rendered terminal images for floating previews.
#[derive(Clone)]
pub struct RenderedDocument {
    pub mermaid_images: HashMap<usize, Protocol>,
    pub markdown_images: HashMap<String, Protocol>,
}

impl RenderedDocument {
    /// Background workers load markdown images and mermaid diagrams on demand.
    pub fn new(
        _document: &Document,
        _picker: &ratatui_image::picker::Picker,
        _terminal: TerminalSize,
        _base_path: Option<&std::path::Path>,
    ) -> Result<Self, AppError> {
        Ok(Self {
            mermaid_images: HashMap::new(),
            markdown_images: HashMap::new(),
        })
    }

    /// Render a mermaid diagram for preview if not already cached.
    ///
    /// Returns `true` when the diagram is available in the cache after this call.
    #[cfg(test)]
    pub fn ensure_mermaid_preview(
        &mut self,
        link_id: usize,
        document: &Document,
        picker: &ratatui_image::picker::Picker,
        terminal: TerminalSize,
    ) -> bool {
        if self.mermaid_images.contains_key(&link_id) {
            return true;
        }
        let Some(link) = document.links.get(link_id) else {
            return false;
        };
        if link.kind != LinkKind::Mermaid {
            return false;
        };
        let Some(diagram_idx) = crate::domain::mermaid_diagram_index(link.url.as_str()) else {
            return false;
        };
        let Some(diag) = document.mermaid_diagrams.get(diagram_idx) else {
            return false;
        };
        match render_mermaid_from_source(&diag.source, picker, terminal) {
            Ok(protocol) => {
                self.mermaid_images.insert(link_id, protocol);
                true
            }
            Err(e) => {
                eprintln!("[bmd] failed to render mermaid link {link_id}: {e}");
                false
            }
        }
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
            LinkKind::Web | LinkKind::Anchor | LinkKind::Document => None,
        }
    }
}

/// Render mermaid source to a terminal image protocol.
pub(crate) fn render_mermaid_from_source(
    source: &str,
    picker: &ratatui_image::picker::Picker,
    terminal: TerminalSize,
) -> Result<Protocol, AppError> {
    render_mermaid_image(
        &MermaidDiagram {
            source: source.into(),
        },
        picker,
        terminal,
    )
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
    use super::*;
    use crate::domain::TerminalSize;
    use crate::domain::{Document, Link, LinkKind, LinkUrl, MermaidDiagram, mermaid_diagram_index};
    use ratatui_image::picker::Picker;

    #[test]
    fn mermaid_diagram_index_parses_bmd_url() {
        assert_eq!(mermaid_diagram_index("bmd:mermaid:0"), Some(0));
        assert_eq!(mermaid_diagram_index("https://x"), None);
    }

    #[test]
    fn new_does_not_preload_mermaid() {
        let document = Document {
            blocks: vec![],
            links: vec![Link {
                url: LinkUrl::new("bmd:mermaid:0".into()).unwrap(),
                title: None,
                kind: LinkKind::Mermaid,
            }],
            mermaid_diagrams: vec![MermaidDiagram {
                source: "graph TD; A-->B;".into(),
            }],
            footnotes: vec![],
            footnote_order: vec![],
            front_matter: None,
        };
        let rendered = RenderedDocument::new(
            &document,
            &Picker::halfblocks(),
            TerminalSize::new(80, 24).unwrap(),
            None,
        )
        .unwrap();
        assert!(rendered.mermaid_images.is_empty());
    }
}
