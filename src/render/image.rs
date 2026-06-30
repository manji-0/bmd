//! Markdown image loading for the floating preview overlay.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    widgets::Widget,
};
use ratatui_image::{FilterType, Resize, protocol::Protocol};

use crate::domain::{Document, LinkKind, TerminalSize};
use crate::error::AppError;

/// Popup size as a percentage of the terminal, matching [`centered_rect`] in preview draw.
pub(crate) const PREVIEW_POPUP_PERCENT: u16 = 85;
const PREVIEW_BORDER_INSET: u16 = 2;

/// Inner content area of the bordered preview popup, in terminal cells.
pub(crate) fn preview_content_size(terminal: TerminalSize) -> Size {
    let w = (terminal.width() as u32 * PREVIEW_POPUP_PERCENT as u32 / 100)
        .saturating_sub(PREVIEW_BORDER_INSET as u32)
        .max(1) as u16;
    let h = (terminal.height() as u32 * PREVIEW_POPUP_PERCENT as u32 / 100)
        .saturating_sub(PREVIEW_BORDER_INSET as u32)
        .max(1) as u16;
    Size::new(w, h)
}

pub(crate) fn load_markdown_image(
    src: &str,
    base_path: Option<&Path>,
) -> Result<DynamicImage, AppError> {
    let path = resolve_image_path(src, base_path)?;
    let bytes = std::fs::read(&path).map_err(AppError::Io)?;
    Ok(image::load_from_memory(&bytes)?)
}

fn resolve_image_path(src: &str, base_path: Option<&Path>) -> Result<PathBuf, AppError> {
    if src.starts_with("http://") || src.starts_with("https://") {
        return Err(AppError::UnsupportedInput(format!(
            "remote images are not supported: {src}"
        )));
    }

    let path_str = src.strip_prefix("file://").unwrap_or(src);
    let path = Path::new(path_str);
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }

    let base = base_path
        .and_then(|p| p.parent())
        .unwrap_or_else(|| Path::new("."));
    Ok(base.join(path))
}

/// Resize filter for preview images (downscale from supersampled raster).
const PREVIEW_RESIZE_FILTER: FilterType = FilterType::Lanczos3;

pub(crate) fn terminal_image_protocol(
    dyn_img: DynamicImage,
    picker: &ratatui_image::picker::Picker,
    target: Size,
) -> Result<Protocol, AppError> {
    let font_size = picker.font_size();
    let resize = Resize::Scale(Some(PREVIEW_RESIZE_FILTER));
    let size = resize.size_for(&dyn_img, font_size, target);
    picker
        .new_protocol(dyn_img, size, resize)
        .map_err(|e| AppError::TerminalImage(e.to_string()))
}

pub(crate) fn preload_markdown_images(
    document: &Document,
    picker: &ratatui_image::picker::Picker,
    terminal: TerminalSize,
    base_path: Option<&Path>,
) -> HashMap<String, Protocol> {
    let target = preview_content_size(terminal);
    let mut images = HashMap::new();
    for link in &document.links {
        if link.kind != LinkKind::Image {
            continue;
        }
        let src = link.url.as_str();
        if images.contains_key(src) {
            continue;
        }
        match load_markdown_image(src, base_path)
            .and_then(|dyn_img| terminal_image_protocol(dyn_img, picker, target))
        {
            Ok(protocol) => {
                images.insert(src.to_string(), protocol);
            }
            Err(e) => {
                eprintln!("[bmd] failed to load image {src}: {e}");
            }
        }
    }
    images
}

pub(crate) fn render_floating_image(protocol: &Protocol, area: Rect, buf: &mut Buffer) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let img_size = protocol.size();
    if img_size.width == 0 || img_size.height == 0 {
        return;
    }
    let render_area = fit_and_center(img_size, area);
    let image = ratatui_image::Image::new(protocol).allow_clipping(true);
    image.render(render_area, buf);
}

fn fit_and_center(image: Size, area: Rect) -> Rect {
    let w = image.width.min(area.width);
    let h = image.height.min(area.height);
    Rect {
        x: area.x + (area.width.saturating_sub(w)) / 2,
        y: area.y + (area.height.saturating_sub(h)) / 2,
        width: w,
        height: h,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use crate::domain::TerminalSize;
    use ratatui::layout::Rect;

    #[test]
    fn preview_content_size_matches_popup_inner_area() {
        let terminal = TerminalSize::new(100, 40).unwrap();
        let size = preview_content_size(terminal);
        assert_eq!(size.width, 83);
        assert_eq!(size.height, 32);
    }

    #[test]
    fn fit_and_center_centers_smaller_image() {
        let area = Rect::new(0, 0, 20, 10);
        let centered = fit_and_center(Size::new(8, 4), area);
        assert_eq!(centered.x, 6);
        assert_eq!(centered.y, 3);
        assert_eq!(centered.width, 8);
        assert_eq!(centered.height, 4);
    }

    #[test]
    fn resolve_relative_image_against_base_path() {
        let base = Path::new("/docs/readme.md");
        let resolved = resolve_image_path("assets/logo.png", Some(base)).unwrap();
        assert_eq!(resolved, Path::new("/docs/assets/logo.png"));
    }

    #[test]
    fn resolve_absolute_image_path() {
        let resolved = resolve_image_path("/tmp/photo.jpg", None).unwrap();
        assert_eq!(resolved, Path::new("/tmp/photo.jpg"));
    }

    #[test]
    fn reject_remote_image_urls() {
        let err = resolve_image_path("https://example.com/a.png", None).unwrap_err();
        assert!(matches!(err, AppError::UnsupportedInput(_)));
    }
}
