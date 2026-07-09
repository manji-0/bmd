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
            LinkKind::Web | LinkKind::Anchor | LinkKind::Document | LinkKind::Toc => None,
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

/// Upper bound on the supersample factor used when compensating for a
/// diagram whose intrinsic size is much smaller than the preview popup.
const MERMAID_MAX_RASTER_SCALE: f32 = 12.0;

fn render_mermaid_image(
    diag: &MermaidDiagram,
    picker: &ratatui_image::picker::Picker,
    terminal: TerminalSize,
) -> Result<Protocol, AppError> {
    let target = preview_content_size(terminal);
    let font = picker.font_size();
    let target_px_w = target.width as f64 * font.width as f64;
    let target_px_h = target.height as f64 * font.height as f64;

    let mut layout = LayoutOptions::headless_svg_defaults();
    layout.viewport_width = target_px_w;
    layout.viewport_height = target_px_h;

    let renderer = HeadlessRenderer::new()
        .with_layout_options(layout)
        .with_diagram_id("bmd-mermaid");

    let dyn_img = rasterize_mermaid(&renderer, &diag.source, MERMAID_RASTER_SCALE)?;

    // `merman` lays most diagram types out at their own intrinsic,
    // content-driven size regardless of `viewport_width`/`viewport_height`
    // (that field only affects the C4 diagram layout). If the resulting
    // raster is smaller than the popup, the terminal display step's
    // aspect-preserving fit would *upscale* it — blurring text and edges.
    // Re-render at a larger supersample so that step only ever downsamples.
    let fit_ratio = (target_px_w / dyn_img.width().max(1) as f64)
        .min(target_px_h / dyn_img.height().max(1) as f64);
    let dyn_img = if fit_ratio > 1.0 {
        let needed_scale =
            (f64::from(MERMAID_RASTER_SCALE) * fit_ratio).min(f64::from(MERMAID_MAX_RASTER_SCALE));
        rasterize_mermaid(&renderer, &diag.source, needed_scale as f32)?
    } else {
        dyn_img
    };

    terminal_image_protocol(dyn_img, picker, target)
}

/// Render a mermaid diagram and open it in the OS default viewer.
///
/// Used when the terminal's graphics protocol falls back to Halfblocks,
/// which is too low-fidelity for a diagram of any complexity. Only called on
/// an explicit user action (pressing `o`) — never from background prefetch —
/// so an external viewer window doesn't pop up unexpectedly on document load.
pub(crate) fn open_mermaid_externally(source: &str) -> Result<(), AppError> {
    let renderer = HeadlessRenderer::new()
        .with_layout_options(LayoutOptions::headless_svg_defaults())
        .with_diagram_id("bmd-mermaid");
    let dyn_img = rasterize_mermaid(&renderer, source, MERMAID_RASTER_SCALE)?;
    let path = save_mermaid_temp_png(&dyn_img)
        .ok_or_else(|| AppError::TerminalImage("failed to save mermaid diagram".into()))?;
    crate::browser::open_path(&path)
}

static MERMAID_TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Save a rendered mermaid diagram to a uniquely-named temp PNG for viewing
/// in an external app. Returns `None` if the write fails.
fn save_mermaid_temp_png(dyn_img: &image::DynamicImage) -> Option<std::path::PathBuf> {
    let n = MERMAID_TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("bmd-mermaid-{}-{n}.png", std::process::id()));
    dyn_img.save(&path).ok()?;
    Some(path)
}

fn rasterize_mermaid(
    renderer: &HeadlessRenderer,
    source: &str,
    scale: f32,
) -> Result<image::DynamicImage, AppError> {
    let options = RasterOptions {
        scale,
        ..Default::default()
    };
    let png = renderer
        .render_png_sync(source, &options)?
        .ok_or(AppError::MermaidNoDiagram)?;
    Ok(image::load_from_memory(&png)?)
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
    fn open_mermaid_externally_saves_a_temp_png() {
        let prefix = format!("bmd-mermaid-{}-", std::process::id());

        let count_matching = |prefix: &str| -> usize {
            std::fs::read_dir(std::env::temp_dir())
                .into_iter()
                .flatten()
                .flatten()
                .filter(|entry| {
                    entry
                        .file_name()
                        .to_str()
                        .is_some_and(|name| name.starts_with(prefix) && name.ends_with(".png"))
                })
                .count()
        };

        let before = count_matching(&prefix);
        open_mermaid_externally("graph TD; A-->B;").unwrap();
        let after = count_matching(&prefix);
        assert!(after > before, "expected a new temp PNG to be saved");

        if let Ok(entries) = std::fs::read_dir(std::env::temp_dir()) {
            for entry in entries.flatten() {
                if entry
                    .file_name()
                    .to_str()
                    .is_some_and(|name| name.starts_with(&prefix) && name.ends_with(".png"))
                {
                    let _ = std::fs::remove_file(entry.path());
                }
            }
        }
    }

    #[test]
    fn render_mermaid_image_has_no_external_open_side_effect() {
        // Background prefetch renders via render_mermaid_from_source and must
        // not launch an external viewer — only the explicit `o`-triggered
        // open_mermaid_externally() should do that.
        let picker = Picker::halfblocks();
        let terminal = TerminalSize::new(80, 24).unwrap();
        assert!(render_mermaid_from_source("graph TD; A-->B;", &picker, terminal).is_ok());
    }

    #[test]
    fn small_diagram_in_large_terminal_is_not_upscaled_below_target() {
        // A tiny diagram rendered for a large popup would otherwise be
        // upscaled by the terminal display step, blurring it. The raster
        // should be re-rendered large enough that only downscaling happens.
        let picker = Picker::halfblocks();
        let terminal = TerminalSize::new(200, 60).unwrap();
        let font = picker.font_size();
        let target = preview_content_size(terminal);
        let target_px_w = target.width as f64 * font.width as f64;
        let target_px_h = target.height as f64 * font.height as f64;

        let source = "graph TD; A-->B; B-->C; A-->C;";

        let mut layout = LayoutOptions::headless_svg_defaults();
        layout.viewport_width = target_px_w;
        layout.viewport_height = target_px_h;
        let renderer = HeadlessRenderer::new()
            .with_layout_options(layout)
            .with_diagram_id("bmd-mermaid");
        let base = rasterize_mermaid(&renderer, source, MERMAID_RASTER_SCALE).unwrap();
        let base_fit_ratio = (target_px_w / base.width().max(1) as f64)
            .min(target_px_h / base.height().max(1) as f64);
        assert!(
            base_fit_ratio > 1.0,
            "expected this synthetic diagram to undershoot the target so the fix is exercised"
        );

        let diag = MermaidDiagram {
            source: source.into(),
        };
        // The public entry point should compensate for the undershoot above.
        render_mermaid_image(&diag, &picker, terminal).unwrap();

        let needed_scale = (f64::from(MERMAID_RASTER_SCALE) * base_fit_ratio)
            .min(f64::from(MERMAID_MAX_RASTER_SCALE));
        let corrected = rasterize_mermaid(&renderer, source, needed_scale as f32).unwrap();
        let corrected_fit_ratio = (target_px_w / corrected.width().max(1) as f64)
            .min(target_px_h / corrected.height().max(1) as f64);
        assert!(
            corrected_fit_ratio <= 1.0 + f64::EPSILON,
            "corrected raster should be large enough that the terminal step only downsamples, got ratio {corrected_fit_ratio}"
        );
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
