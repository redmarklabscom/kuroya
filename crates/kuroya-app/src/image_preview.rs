use crate::file_io::{FILE_OPEN_MAX_BYTES, format_byte_size, read_file_bytes_with_limit};
use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Rect, TextureOptions, Ui, pos2, vec2,
};
use image::{ImageReader, Limits};
use std::{io::Cursor, path::Path};

const IMAGE_PREVIEW_MAX_PIXELS: u64 = 64_000_000;
const IMAGE_PREVIEW_MAX_SIDE: u32 = 32_768;
const IMAGE_PREVIEW_MARGIN: f32 = 24.0;
const IMAGE_PREVIEW_METADATA_PADDING: f32 = 10.0;

#[derive(Debug, Clone)]
pub(crate) struct LoadedImagePreview {
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) rgba: Vec<u8>,
    pub(crate) byte_len: usize,
}

pub(crate) struct ImagePreviewState {
    loaded: LoadedImagePreview,
    texture: Option<egui::TextureHandle>,
}

impl ImagePreviewState {
    pub(crate) fn from_loaded(loaded: LoadedImagePreview) -> Self {
        Self {
            loaded,
            texture: None,
        }
    }

    fn texture_id(
        &mut self,
        ctx: &egui::Context,
        buffer_id: kuroya_core::BufferId,
    ) -> egui::TextureId {
        let loaded = &self.loaded;
        self.texture
            .get_or_insert_with(|| {
                let image =
                    ColorImage::from_rgba_unmultiplied([loaded.width, loaded.height], &loaded.rgba);
                ctx.load_texture(
                    format!("kuroya-image-preview-{buffer_id}"),
                    image,
                    TextureOptions::LINEAR,
                )
            })
            .id()
    }
}

pub(crate) fn path_is_image_preview(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(image_extension_is_supported)
}

pub(crate) async fn load_image_preview(path: &Path) -> Result<LoadedImagePreview, String> {
    let bytes = read_file_bytes_with_limit(path, FILE_OPEN_MAX_BYTES).await?;
    tokio::task::spawn_blocking(move || decode_image_preview(bytes))
        .await
        .map_err(|error| format!("image preview task failed: {error}"))?
}

pub(crate) fn image_preview_buffer_text(preview: &LoadedImagePreview) -> String {
    format!(
        "Image preview\n{} x {} px\n{}\n",
        preview.width,
        preview.height,
        format_byte_size(preview.byte_len as u64)
    )
}

pub(crate) fn image_preview_status_detail(preview: &LoadedImagePreview) -> String {
    format!(
        "{} x {} px, {}",
        preview.width,
        preview.height,
        format_byte_size(preview.byte_len as u64)
    )
}

pub(crate) fn render_image_preview(
    ui: &mut Ui,
    viewport_rect: Rect,
    buffer_id: kuroya_core::BufferId,
    preview: &mut ImagePreviewState,
    font_size: f32,
) {
    if !viewport_rect.is_positive() {
        return;
    }

    let texture_id = preview.texture_id(ui.ctx(), buffer_id);
    let image_rect = fitted_image_preview_rect(
        viewport_rect.shrink(IMAGE_PREVIEW_MARGIN),
        [preview.loaded.width, preview.loaded.height],
    );
    if image_rect.is_positive() {
        ui.painter().image(
            texture_id,
            image_rect,
            Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );
    }

    let metadata = image_preview_status_detail(&preview.loaded);
    let metadata_y = (image_rect.bottom() + IMAGE_PREVIEW_METADATA_PADDING)
        .min(viewport_rect.bottom() - IMAGE_PREVIEW_METADATA_PADDING);
    ui.painter().text(
        pos2(viewport_rect.center().x, metadata_y),
        Align2::CENTER_TOP,
        metadata,
        FontId::monospace((font_size * 0.84).clamp(10.0, 13.0)),
        Color32::from_rgb(175, 179, 186),
    );
}

fn image_extension_is_supported(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "bmp" | "gif" | "ico" | "jpeg" | "jpg" | "jfif" | "png" | "tif" | "tiff" | "webp"
    )
}

fn decode_image_preview(bytes: Vec<u8>) -> Result<LoadedImagePreview, String> {
    let byte_len = bytes.len();
    let mut reader = ImageReader::new(Cursor::new(bytes));
    let mut limits = Limits::default();
    limits.max_image_width = Some(IMAGE_PREVIEW_MAX_SIDE);
    limits.max_image_height = Some(IMAGE_PREVIEW_MAX_SIDE);
    limits.max_alloc = Some(IMAGE_PREVIEW_MAX_PIXELS.saturating_mul(4));
    reader.limits(limits);
    let image = reader
        .with_guessed_format()
        .map_err(|error| format!("could not identify image format: {error}"))?
        .decode()
        .map_err(|error| format!("could not decode image: {error}"))?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    validate_image_dimensions(width, height)?;
    Ok(LoadedImagePreview {
        width: width as usize,
        height: height as usize,
        rgba: rgba.into_raw(),
        byte_len,
    })
}

fn validate_image_dimensions(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("image has empty dimensions".to_owned());
    }
    let pixels = u64::from(width).saturating_mul(u64::from(height));
    if pixels > IMAGE_PREVIEW_MAX_PIXELS {
        Err(format!(
            "image is too large to preview ({} x {} px)",
            width, height
        ))
    } else {
        Ok(())
    }
}

fn fitted_image_preview_rect(bounds: Rect, image_size: [usize; 2]) -> Rect {
    if !bounds.is_positive() || image_size[0] == 0 || image_size[1] == 0 {
        return Rect::from_center_size(bounds.center(), egui::Vec2::ZERO);
    }

    let image_width = image_size[0] as f32;
    let image_height = image_size[1] as f32;
    if !image_width.is_finite() || !image_height.is_finite() {
        return Rect::from_center_size(bounds.center(), egui::Vec2::ZERO);
    }

    let scale = (bounds.width() / image_width)
        .min(bounds.height() / image_height)
        .clamp(0.0, 1.0);
    let size = vec2(image_width * scale, image_height * scale);
    Rect::from_center_size(bounds.center(), size)
}

#[cfg(test)]
mod tests {
    use super::{
        decode_image_preview, fitted_image_preview_rect, path_is_image_preview,
        validate_image_dimensions,
    };
    use eframe::egui::{Rect, pos2};
    use std::{io::Cursor, path::Path};

    #[test]
    fn image_preview_detects_supported_extensions_case_insensitively() {
        assert!(path_is_image_preview(Path::new("photo.JPG")));
        assert!(path_is_image_preview(Path::new("icon.png")));
        assert!(path_is_image_preview(Path::new("scan.tiff")));
        assert!(!path_is_image_preview(Path::new("vector.svg")));
        assert!(!path_is_image_preview(Path::new("main.rs")));
    }

    #[test]
    fn image_preview_dimension_guard_rejects_empty_or_huge_images() {
        assert!(validate_image_dimensions(1, 1).is_ok());
        assert!(validate_image_dimensions(0, 10).is_err());
        assert!(validate_image_dimensions(100_000, 100_000).is_err());
    }

    #[test]
    fn image_preview_decode_returns_rgba_pixels() {
        let image = image::RgbaImage::from_raw(1, 1, vec![12, 34, 56, 255])
            .expect("test image should be valid");
        let mut bytes = Cursor::new(Vec::new());
        image::DynamicImage::ImageRgba8(image)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .expect("test image should encode");

        let preview = decode_image_preview(bytes.into_inner()).expect("png should decode");

        assert_eq!(preview.width, 1);
        assert_eq!(preview.height, 1);
        assert_eq!(preview.rgba, vec![12, 34, 56, 255]);
        assert!(preview.byte_len > 0);
    }

    #[test]
    fn fitted_image_preview_rect_centers_and_downscales_without_upscaling() {
        let bounds = Rect::from_min_max(pos2(0.0, 0.0), pos2(200.0, 100.0));

        let large = fitted_image_preview_rect(bounds, [400, 100]);
        assert_eq!(large.width(), 200.0);
        assert_eq!(large.height(), 50.0);
        assert_eq!(large.center(), bounds.center());

        let small = fitted_image_preview_rect(bounds, [50, 20]);
        assert_eq!(small.width(), 50.0);
        assert_eq!(small.height(), 20.0);
        assert_eq!(small.center(), bounds.center());
    }
}
