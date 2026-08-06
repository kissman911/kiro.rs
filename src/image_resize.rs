//! 入站图片降采样与重编码
//!
//! 把 Anthropic 协议 ContentBlock 里携带的 base64 图片，在中转层**本地 CPU** 降采样到
//! 长边 <= `KIRO_RS_IMAGE_MAX_LONG_SIDE` px、字节 <= `KIRO_RS_IMAGE_MAX_BYTES`，
//! 再重编码回 base64 写回 KiroImage。为什么需要这一步：
//!
//! 1. AWS Q（`q.us-east-1.amazonaws.com`）后端对单字段有硬上限。一张 iPhone 截图
//!    （1206x2622 PNG）单条 base64 约 700K 字符会直接触发 `CONTENT_LENGTH_EXCEEDS_THRESHOLD`。
//! 2. Anthropic 建议长边 <= 1568 px；这是视觉编码器 patch 网格边界。超过后服务端会再缩，
//!    但 token 仍按原图计费。
//! 3. ChatGPT/OpenAI 服务端自动缩到该尺寸，AWS Q 不缩——这正是同一张 iPhone 截图在 GPT
//!    上能过、Kiro Opus 却 400 的根因。
//!
//! 设计原则：
//! - 小图直通（不解码、不重编码，零开销）
//! - 大图降采样到长边上限并重编码为 JPEG（PNG/WebP/JPEG 都出 JPEG；GIF 例外，保留原格式，可能是动图）
//! - 解码失败**保留原图**并打 warning；坏图绝不拖垮整个请求
//! - 全部由 `KIRO_RS_IMAGE_*` 环境变量驱动

use std::io::Cursor;

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use image::{ImageFormat, ImageReader, imageops::FilterType};
use tracing::{debug, warn};

/// 默认长边阈值（Anthropic 推荐值）
const DEFAULT_MAX_LONG_SIDE: u32 = 1568;
/// 默认字节阈值（在 AWS Q 单字段上限下留安全余量）
const DEFAULT_MAX_BYTES: usize = 400_000;
/// 默认 JPEG 质量
const DEFAULT_JPEG_QUALITY: u8 = 85;

/// 入站图片处理配置
#[derive(Debug, Clone, Copy)]
pub struct ResizeConfig {
    pub enabled: bool,
    pub max_long_side: u32,
    pub max_bytes: usize,
    pub jpeg_quality: u8,
}

impl ResizeConfig {
    /// 从 `KIRO_RS_IMAGE_*` 环境变量读取，未设置则回落默认值
    pub fn from_env() -> Self {
        let enabled = !matches!(
            std::env::var("KIRO_RS_IMAGE_RESIZE")
                .unwrap_or_else(|_| "1".to_string())
                .to_ascii_lowercase()
                .as_str(),
            "0" | "false" | "no" | "off"
        );
        let max_long_side = std::env::var("KIRO_RS_IMAGE_MAX_LONG_SIDE")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_LONG_SIDE);
        let max_bytes = std::env::var("KIRO_RS_IMAGE_MAX_BYTES")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_MAX_BYTES);
        let jpeg_quality = std::env::var("KIRO_RS_IMAGE_JPEG_QUALITY")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(DEFAULT_JPEG_QUALITY);
        Self {
            enabled,
            max_long_side,
            max_bytes,
            jpeg_quality,
        }
    }
}

/// 单张图片处理结果（显式区分「原样保留」与「已重编码」两种状态）
///
/// `was_resized` / `original_bytes` / `final_bytes` 仅被测试断言与结构化日志消费；
/// 运行期非测试路径不读它们，故整个结构标 `allow(dead_code)` 保留诊断字段。
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct ProcessedImage {
    /// 输出格式（"jpeg" / "png" / "gif" / "webp"）
    pub format: String,
    /// 输出 base64 字符串
    pub data_base64: String,
    /// 是否真的发生了重编码（用于日志/指标）
    pub was_resized: bool,
    /// 输入字节数（解码前）
    pub original_bytes: usize,
    /// 输出字节数
    pub final_bytes: usize,
}

/// 主入口：按「够小就直通 / 过大就缩」规则处理单张入站图片
///
/// `format` 是源 media-type 的最后一段（"png" / "jpeg" / "gif" / "webp"），
/// `data_base64` 是 base64 编码的原始字节。
///
/// 绝不 panic、绝不丢图。失败时返回输入的拥有副本并打 warning。
pub fn maybe_shrink_image(cfg: ResizeConfig, format: &str, data_base64: &str) -> ProcessedImage {
    let format_lc = format.to_ascii_lowercase();
    let original_bytes = data_base64.len();

    // 1) 禁用：原样返回
    if !cfg.enabled {
        return passthrough(format_lc, data_base64);
    }
    // 2) 字节够小：原样返回（小图无需处理，省 CPU）
    if data_base64.len() <= cfg.max_bytes {
        // 即便字节小，也查一下尺寸是否超大（罕见，如 7000x100 横幅）
        if let Some((w, h)) = peek_dimensions(&format_lc, data_base64)
            && w.max(h) <= cfg.max_long_side
        {
            return passthrough(format_lc, data_base64);
        }
        // 字节小但尺寸超大：仍走重编码路径
    }
    // 3) 动图（多帧 GIF）保留原格式不变——转 JPEG 会丢动画
    if format_lc == "gif" {
        debug!(
            target: "kiro_rs::image_resize",
            original_bytes = original_bytes,
            "skip GIF (potential animation)"
        );
        return passthrough(format_lc, data_base64);
    }

    // 4) 真正缩图
    match shrink_static_image(cfg, &format_lc, data_base64) {
        Ok(processed) => processed,
        Err(e) => {
            warn!(
                target: "kiro_rs::image_resize",
                error = %e,
                format = %format_lc,
                original_bytes = original_bytes,
                "image resize failed; passing through original"
            );
            passthrough(format_lc, data_base64)
        }
    }
}

fn passthrough(format: String, data_base64: &str) -> ProcessedImage {
    let n = data_base64.len();
    // 用真实 magic bytes 校正格式：宿主可能标了 png 但字节其实是 jpeg，
    // 忠实直通会触发 Bedrock 严格 MIME 检查 IMAGE_MIME_MISMATCH。检测失败则保留原标签（绝不丢图）。
    let format = match detect_format_from_bytes(data_base64) {
        Some(real) if real != format => {
            debug!(
                target: "kiro_rs::image_resize",
                declared = %format,
                actual = %real,
                "passthrough format corrected from magic bytes"
            );
            real
        }
        _ => format,
    };
    ProcessedImage {
        format,
        data_base64: data_base64.to_string(),
        was_resized: false,
        original_bytes: n,
        final_bytes: n,
    }
}

/// 从真实 magic bytes 检测格式，返回 "png"/"jpeg"/"gif"/"webp"。
/// 只解前 ~16 字节（前 24 个 base64 字符）足够覆盖所有 magic number 且省 CPU。
/// 检测失败（解码错误 / 未知格式）返回 None，调用方安全保留原标签。
fn detect_format_from_bytes(data_base64: &str) -> Option<String> {
    let head: String = data_base64.chars().take(24).collect();
    let bytes = BASE64.decode(head.as_bytes()).ok()?;
    match image::guess_format(&bytes).ok()? {
        ImageFormat::Png => Some("png".to_string()),
        ImageFormat::Jpeg => Some("jpeg".to_string()),
        ImageFormat::Gif => Some("gif".to_string()),
        ImageFormat::WebP => Some("webp".to_string()),
        _ => None,
    }
}

/// 只读头部拿尺寸，不解码全部像素；每张 < 1ms
fn peek_dimensions(format: &str, data_base64: &str) -> Option<(u32, u32)> {
    let bytes = BASE64.decode(data_base64).ok()?;
    let cursor = Cursor::new(&bytes);
    let mut reader = ImageReader::new(cursor);
    if let Some(fmt) = guess_format(format) {
        reader.set_format(fmt);
    } else {
        reader = reader.with_guessed_format().ok()?;
    }
    reader.into_dimensions().ok()
}

fn guess_format(s: &str) -> Option<ImageFormat> {
    match s {
        "png" => Some(ImageFormat::Png),
        "jpeg" | "jpg" => Some(ImageFormat::Jpeg),
        "webp" => Some(ImageFormat::WebP),
        "gif" => Some(ImageFormat::Gif),
        _ => None,
    }
}

fn shrink_static_image(
    cfg: ResizeConfig,
    format: &str,
    data_base64: &str,
) -> Result<ProcessedImage, ResizeError> {
    let original_bytes = data_base64.len();

    let raw = BASE64
        .decode(data_base64)
        .map_err(|e| ResizeError::Base64(e.to_string()))?;

    let cursor = Cursor::new(&raw);
    let mut reader = ImageReader::new(cursor);
    if let Some(fmt) = guess_format(format) {
        reader.set_format(fmt);
    } else {
        reader = reader
            .with_guessed_format()
            .map_err(|e| ResizeError::Decode(e.to_string()))?;
    }
    let img = reader
        .decode()
        .map_err(|e| ResizeError::Decode(e.to_string()))?;

    // 初次按配置长边等比缩放（保持纵横比）。
    let (w, h) = (img.width(), img.height());
    let long_initial = w.max(h);
    let mut cur_long = long_initial.min(cfg.max_long_side).max(1);

    // 两级收敛以满足 max_bytes：对每个长边上限，先按配置质量编码再逐步降质量；
    // 若到最低质量仍不达标，进一步缩长边重试。保证输出真的落进 max_bytes（到一个小下限），
    // 而不是返回超大数据。
    const MIN_JPEG_QUALITY: u8 = 35;
    const MIN_LONG_SIDE: u32 = 256;
    let mut out;
    let mut quality;
    loop {
        let resized = if w.max(h) > cur_long {
            let scale = cur_long as f32 / w.max(h) as f32;
            let new_w = ((w as f32) * scale).round().max(1.0) as u32;
            let new_h = ((h as f32) * scale).round().max(1.0) as u32;
            // Lanczos3 视觉质量好；1206x2622 -> 1024x~470 单核约 80ms。
            img.resize_exact(new_w, new_h, FilterType::Lanczos3)
        } else {
            img.clone()
        };
        // 强制 RGB8（JPEG 无 alpha；截图丢 alpha 无害）。
        let rgb = resized.to_rgb8();
        quality = cfg.jpeg_quality;
        loop {
            out = Vec::with_capacity(64 * 1024);
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality);
            rgb.write_with_encoder(encoder)
                .map_err(|e| ResizeError::Encode(e.to_string()))?;
            // base64 膨胀约 4/3；编码后 base64 长度落进预算即停。
            if out.len().saturating_mul(4) / 3 <= cfg.max_bytes || quality <= MIN_JPEG_QUALITY {
                break;
            }
            quality = quality.saturating_sub(10).max(MIN_JPEG_QUALITY);
        }
        if out.len().saturating_mul(4) / 3 <= cfg.max_bytes || cur_long <= MIN_LONG_SIDE {
            break;
        }
        // 质量触底仍超标：缩长边重试。
        cur_long = ((cur_long as f32 * 0.8) as u32).max(MIN_LONG_SIDE);
    }
    let final_bytes_raw = out.len();
    let data_b64 = BASE64.encode(&out);
    let final_bytes = data_b64.len();

    debug!(
        target: "kiro_rs::image_resize",
        original_bytes = original_bytes,
        final_bytes = final_bytes,
        ratio = format!("{:.2}x", original_bytes as f64 / final_bytes.max(1) as f64),
        decoded_w = w,
        decoded_h = h,
        out_jpeg_bytes = final_bytes_raw,
        "image resized"
    );

    Ok(ProcessedImage {
        format: "jpeg".to_string(),
        data_base64: data_b64,
        was_resized: true,
        original_bytes,
        final_bytes,
    })
}

#[derive(Debug, thiserror::Error)]
enum ResizeError {
    #[error("base64 decode: {0}")]
    Base64(String),
    #[error("image decode: {0}")]
    Decode(String),
    #[error("image encode: {0}")]
    Encode(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_png(w: u32, h: u32) -> String {
        use image::{Rgb, RgbImage};
        let mut img = RgbImage::new(w, h);
        // 渐变填充：压缩比比纯色更接近真实截图
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
            }
        }
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Png)
            .unwrap();
        BASE64.encode(&buf)
    }

    fn make_jpeg(w: u32, h: u32) -> String {
        use image::{Rgb, RgbImage};
        let mut img = RgbImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                img.put_pixel(x, y, Rgb([(x % 256) as u8, (y % 256) as u8, 128]));
            }
        }
        let mut buf = Vec::new();
        img.write_to(&mut Cursor::new(&mut buf), ImageFormat::Jpeg)
            .unwrap();
        BASE64.encode(&buf)
    }

    #[test]
    fn small_image_passes_through() {
        let cfg = ResizeConfig {
            enabled: true,
            max_long_side: 1568,
            max_bytes: 400_000,
            jpeg_quality: 85,
        };
        let small = make_png(64, 64);
        let out = maybe_shrink_image(cfg, "png", &small);
        assert!(!out.was_resized);
        assert_eq!(out.format, "png");
        assert_eq!(out.data_base64, small);
    }

    #[test]
    fn iphone_screenshot_gets_shrunk_below_limit() {
        let cfg = ResizeConfig {
            enabled: true,
            max_long_side: 1568,
            max_bytes: 400_000,
            jpeg_quality: 85,
        };
        // 1206x2622 ~ iPhone Pro Max 截图比例
        let big = make_png(1206, 2622);
        let out = maybe_shrink_image(cfg, "png", &big);
        assert!(out.was_resized, "should have been resized");
        assert_eq!(out.format, "jpeg", "should have been re-encoded as JPEG");
        assert!(
            out.final_bytes < cfg.max_bytes,
            "final {} should be < cap {}",
            out.final_bytes,
            cfg.max_bytes
        );
        let _ = out.original_bytes;
    }

    #[test]
    fn within_dimensions_but_oversized_bytes_converges_under_cap() {
        // 尺寸在 max_long_side 内，跳过缩放分支；只能靠编码循环里的逐步降质量满足 max_bytes。
        let cfg = ResizeConfig {
            enabled: true,
            max_long_side: 1568,
            max_bytes: 20_000,
            jpeg_quality: 85,
        };
        let img = make_png(1024, 1024);
        let out = maybe_shrink_image(cfg, "png", &img);
        assert!(out.was_resized, "should have been re-encoded");
        assert!(
            out.final_bytes <= cfg.max_bytes,
            "final {} must be <= cap {} after quality reduction",
            out.final_bytes,
            cfg.max_bytes
        );
    }

    #[test]
    fn gif_passes_through_to_preserve_animation() {
        let cfg = ResizeConfig::from_env();
        let tiny_gif = "R0lGODlhAQABAAAAACw=";
        let out = maybe_shrink_image(cfg, "gif", tiny_gif);
        assert!(!out.was_resized);
        assert_eq!(out.format, "gif");
    }

    #[test]
    fn disabled_config_passes_through_even_huge() {
        let cfg = ResizeConfig {
            enabled: false,
            max_long_side: 1568,
            max_bytes: 400_000,
            jpeg_quality: 85,
        };
        let big = make_png(1206, 2622);
        let out = maybe_shrink_image(cfg, "png", &big);
        assert!(!out.was_resized);
        assert_eq!(out.format, "png");
    }

    #[test]
    fn corrupt_data_passes_through_with_warning() {
        let cfg = ResizeConfig {
            enabled: true,
            max_long_side: 1568,
            max_bytes: 100,
            jpeg_quality: 85,
        };
        let bogus = "X".repeat(1000);
        let out = maybe_shrink_image(cfg, "png", &bogus);
        assert!(!out.was_resized, "corrupt input should fall through");
        assert_eq!(out.format, "png");
        assert_eq!(out.data_base64, bogus);
    }

    #[test]
    fn mislabeled_png_header_jpeg_bytes_corrected_to_jpeg() {
        let cfg = ResizeConfig {
            enabled: true,
            max_long_side: 1568,
            max_bytes: 400_000,
            jpeg_quality: 85,
        };
        // 真 JPEG 字节但被标 format="png"（宿主侧头/体不符，忠实直通）。
        // 小图走直通路径。出方向格式必须按真实字节校正为 jpeg，否则 Bedrock 返回 IMAGE_MIME_MISMATCH。
        let jpeg = make_jpeg(64, 64);
        let out = maybe_shrink_image(cfg, "png", &jpeg);
        assert_eq!(out.data_base64, jpeg, "must not mutate image bytes");
        assert_eq!(
            out.format, "jpeg",
            "format must be corrected to match actual JPEG bytes"
        );
    }

    #[test]
    fn matching_png_kept_as_png() {
        let cfg = ResizeConfig::from_env();
        let png = make_png(64, 64);
        let out = maybe_shrink_image(cfg, "png", &png);
        assert_eq!(out.format, "png", "real png must stay png");
        assert_eq!(out.data_base64, png);
    }

    #[test]
    fn matching_jpeg_kept_as_jpeg() {
        let cfg = ResizeConfig::from_env();
        let jpeg = make_jpeg(64, 64);
        let out = maybe_shrink_image(cfg, "jpeg", &jpeg);
        assert_eq!(out.format, "jpeg", "real jpeg must stay jpeg");
        assert_eq!(out.data_base64, jpeg);
    }

    #[test]
    fn undetectable_bytes_keep_declared_format() {
        // 坏数据检测失败 -> 保留入站格式，绝不丢图。
        let cfg = ResizeConfig {
            enabled: false,
            max_long_side: 1568,
            max_bytes: 400_000,
            jpeg_quality: 85,
        };
        let bogus = "X".repeat(40);
        let out = maybe_shrink_image(cfg, "png", &bogus);
        assert_eq!(out.format, "png", "undetectable bytes keep declared format");
        assert_eq!(out.data_base64, bogus);
    }
}
