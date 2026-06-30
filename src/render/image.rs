//! Markdown image loading and rendering.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use image::DynamicImage;
use ratatui::{
    buffer::Buffer,
    layout::{Rect, Size},
    text::{Line, Text},
    widgets::{Paragraph, Widget},
};
use ratatui_image::{Resize, protocol::Protocol};

use crate::domain::{Block, Document, MarkdownImage};
use crate::error::AppError;

use super::context::RenderContext;

pub(crate) fn collect_markdown_images<'a>(blocks: &'a [Block], out: &mut Vec<&'a MarkdownImage>) {
    for block in blocks {
        match block {
            Block::Image(img) => out.push(img),
            Block::BlockQuote(children) => collect_markdown_images(children, out),
            Block::List(list) => {
                for item in &list.items {
                    collect_markdown_images(&item.content, out);
                }
            }
            _ => {}
        }
    }
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

pub(crate) fn terminal_image_protocol(
    dyn_img: DynamicImage,
    picker: &ratatui_image::picker::Picker,
    max_width: u16,
) -> Result<Protocol, AppError> {
    let font_size = picker.font_size();
    let cols = max_width.max(20) as u32;
    let rows = dyn_img
        .height()
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

pub(crate) fn preload_markdown_images(
    document: &Document,
    picker: &ratatui_image::picker::Picker,
    width: u16,
    base_path: Option<&Path>,
) -> HashMap<String, Protocol> {
    let mut images = HashMap::new();
    let mut pending = Vec::new();
    collect_markdown_images(&document.blocks, &mut pending);
    for img in pending {
        if images.contains_key(&img.src) {
            continue;
        }
        match load_markdown_image(&img.src, base_path)
            .and_then(|dyn_img| terminal_image_protocol(dyn_img, picker, width))
        {
            Ok(protocol) => {
                images.insert(img.src.clone(), protocol);
            }
            Err(e) => {
                eprintln!("[bmd] failed to load image {}: {e}", img.src);
            }
        }
    }
    images
}

pub(crate) fn measure_image_height(img: &MarkdownImage, ctx: &RenderContext, width: u16) -> usize {
    if let Some(protocol) = ctx.rendered.markdown_images.get(&img.src) {
        return protocol.size().height as usize;
    }
    let cols = (width as usize).min(160);
    (cols * 9).div_ceil(16).max(6)
}

pub(crate) fn render_markdown_image(
    img: &MarkdownImage,
    area: Rect,
    buf: &mut Buffer,
    _skip_rows: usize,
    ctx: &RenderContext,
) {
    if let Some(protocol) = ctx.rendered.markdown_images.get(&img.src) {
        let image = ratatui_image::Image::new(protocol).allow_clipping(true);
        image.render(area, buf);
        return;
    }

    let label = if img.alt.is_empty() {
        format!("[image: {}]", img.src)
    } else {
        format!("[image: {}]", img.alt)
    };
    let placeholder = Paragraph::new(Text::from(vec![Line::styled(
        label,
        ctx.theme.mermaid_placeholder,
    )]));
    placeholder.render(area, buf);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
