//! Image checks against Bedrock's limits (port of `imageValidation.ts`).

use serde::{Deserialize, Serialize};

use crate::file_naming::node_extname;
use crate::types::{IMAGE_EXTENSIONS, MAX_IMAGES, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BedrockImageFormat {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl BedrockImageFormat {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpeg",
            Self::Gif => "gif",
            Self::Webp => "webp",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDimensions {
    pub width: u32,
    pub height: u32,
}

pub fn is_image_extension(file_name: &str) -> bool {
    IMAGE_EXTENSIONS.contains(&node_extname(file_name).to_lowercase().as_str())
}

/// Bedrock's image format names, which are not simply the extension: `.jpg` must be `jpeg`.
pub fn to_bedrock_image_format(file_name: &str) -> Option<BedrockImageFormat> {
    match node_extname(file_name).to_lowercase().as_str() {
        ".png" => Some(BedrockImageFormat::Png),
        ".jpg" | ".jpeg" => Some(BedrockImageFormat::Jpeg),
        ".gif" => Some(BedrockImageFormat::Gif),
        ".webp" => Some(BedrockImageFormat::Webp),
        _ => None,
    }
}

fn u16_be(b: &[u8], at: usize) -> u32 {
    u32::from(u16::from_be_bytes([b[at], b[at + 1]]))
}

fn u16_le(b: &[u8], at: usize) -> u32 {
    u32::from(u16::from_le_bytes([b[at], b[at + 1]]))
}

fn u32_be(b: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

fn u32_le(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

const PNG_SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];

/// Read pixel dimensions from the file header for PNG, GIF, WebP and JPEG.
///
/// Returns `None` for an unrecognized or truncated header, in which case callers skip the
/// dimension check rather than reject the file.
pub fn read_image_dimensions(buffer: &[u8]) -> Option<ImageDimensions> {
    let dims = |width, height| Some(ImageDimensions { width, height });

    // PNG: 8-byte signature, then an IHDR chunk whose data starts at byte 16.
    if buffer.len() >= 24 && buffer[0..8] == PNG_SIGNATURE {
        return dims(u32_be(buffer, 16), u32_be(buffer, 20));
    }

    // GIF: "GIF87a"/"GIF89a" then the logical screen descriptor, little endian.
    if buffer.len() >= 10 && &buffer[0..3] == b"GIF" {
        return dims(u16_le(buffer, 6), u16_le(buffer, 8));
    }

    // WebP: RIFF container; VP8X, VP8 (lossy) and VP8L (lossless) store size differently.
    if buffer.len() >= 30 && &buffer[0..4] == b"RIFF" && &buffer[8..12] == b"WEBP" {
        let b = |i: usize| u32::from(buffer[i]);
        return match &buffer[12..16] {
            b"VP8X" => dims(
                1 + (b(24) | (b(25) << 8) | (b(26) << 16)),
                1 + (b(27) | (b(28) << 8) | (b(29) << 16)),
            ),
            b"VP8 " => dims(u16_le(buffer, 26) & 0x3fff, u16_le(buffer, 28) & 0x3fff),
            b"VP8L" => {
                let bits = u32_le(buffer, 21);
                dims(1 + (bits & 0x3fff), 1 + ((bits >> 14) & 0x3fff))
            }
            _ => None,
        };
    }

    // JPEG: scan the marker segments for a start-of-frame, which carries the dimensions.
    if buffer.len() >= 4 && buffer[0] == 0xff && buffer[1] == 0xd8 {
        let mut offset = 2;
        while offset + 9 < buffer.len() {
            if buffer[offset] != 0xff {
                offset += 1;
                continue;
            }
            let marker = buffer[offset + 1];
            // SOF0-3, SOF5-7, SOF9-11, SOF13-15 carry height/width at the same offset.
            let is_start_of_frame = (0xc0..=0xcf).contains(&marker)
                && marker != 0xc4
                && marker != 0xc8
                && marker != 0xcc;
            if is_start_of_frame {
                return dims(u16_be(buffer, offset + 7), u16_be(buffer, offset + 5));
            }
            let segment_length = u16_be(buffer, offset + 2) as usize;
            if segment_length == 0 {
                return None;
            }
            offset += 2 + segment_length;
        }
    }

    None
}

/// Validate image bytes against Bedrock's limits.
/// Returns an error message, or `None` when the bytes are acceptable.
pub fn validate_image_bytes(
    file_name: &str,
    bytes: &[u8],
    existing_image_count: usize,
) -> Option<String> {
    if to_bedrock_image_format(file_name).is_none() {
        let ext = node_extname(file_name);
        let ext = if ext.is_empty() { "unknown" } else { ext };
        return Some(format!("Unsupported image format: {ext}"));
    }
    if existing_image_count >= MAX_IMAGES {
        return Some(format!(
            "This chat already has the maximum of {MAX_IMAGES} images attached."
        ));
    }
    if bytes.len() > MAX_IMAGE_BYTES {
        return Some("Image is larger than the 3.75 MB limit Bedrock accepts.".to_string());
    }

    if let Some(d) = read_image_dimensions(bytes) {
        if d.width > MAX_IMAGE_DIMENSION || d.height > MAX_IMAGE_DIMENSION {
            return Some(format!(
                "Image is larger than {MAX_IMAGE_DIMENSION}px on a side ({}x{}).",
                d.width, d.height
            ));
        }
    }
    None
}

#[cfg(test)]
pub(crate) mod tests {
    //! Port of `imageValidation.test.ts`.
    use super::*;

    /// Minimal 8-byte PNG signature followed by an IHDR chunk carrying the size.
    pub(crate) fn png_header(width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; 24];
        buffer[0..8].copy_from_slice(&PNG_SIGNATURE);
        buffer[12..16].copy_from_slice(b"IHDR");
        buffer[16..20].copy_from_slice(&width.to_be_bytes());
        buffer[20..24].copy_from_slice(&height.to_be_bytes());
        buffer
    }

    fn gif_header(width: u16, height: u16) -> Vec<u8> {
        let mut buffer = vec![0u8; 10];
        buffer[0..6].copy_from_slice(b"GIF89a");
        buffer[6..8].copy_from_slice(&width.to_le_bytes());
        buffer[8..10].copy_from_slice(&height.to_le_bytes());
        buffer
    }

    /// JPEG with one APP0 segment before the SOF0 that carries the size.
    fn jpeg_header(width: u16, height: u16) -> Vec<u8> {
        let mut buffer = vec![0u8; 40];
        buffer[0..2].copy_from_slice(&0xffd8u16.to_be_bytes()); // SOI
        buffer[2..4].copy_from_slice(&0xffe0u16.to_be_bytes()); // APP0
        buffer[4..6].copy_from_slice(&6u16.to_be_bytes()); // segment length
        buffer[10..12].copy_from_slice(&0xffc0u16.to_be_bytes()); // SOF0
        buffer[12..14].copy_from_slice(&11u16.to_be_bytes()); // segment length
        buffer[14] = 8; // precision
        buffer[15..17].copy_from_slice(&height.to_be_bytes());
        buffer[17..19].copy_from_slice(&width.to_be_bytes());
        buffer
    }

    fn webp_vp8x_header(width: u32, height: u32) -> Vec<u8> {
        let mut buffer = vec![0u8; 30];
        buffer[0..4].copy_from_slice(b"RIFF");
        buffer[8..12].copy_from_slice(b"WEBP");
        buffer[12..16].copy_from_slice(b"VP8X");
        buffer[24..27].copy_from_slice(&(width - 1).to_le_bytes()[0..3]);
        buffer[27..30].copy_from_slice(&(height - 1).to_le_bytes()[0..3]);
        buffer
    }

    fn d(width: u32, height: u32) -> Option<ImageDimensions> {
        Some(ImageDimensions { width, height })
    }

    mod read_image_dimensions {
        use super::*;

        #[test]
        fn reads_a_png_header() {
            assert_eq!(read_image_dimensions(&png_header(1200, 800)), d(1200, 800));
        }

        #[test]
        fn reads_a_gif_header() {
            assert_eq!(read_image_dimensions(&gif_header(64, 48)), d(64, 48));
        }

        #[test]
        fn reads_a_jpeg_start_of_frame_past_an_earlier_segment() {
            assert_eq!(
                read_image_dimensions(&jpeg_header(4032, 3024)),
                d(4032, 3024)
            );
        }

        #[test]
        fn reads_a_webp_vp8x_canvas_size() {
            assert_eq!(
                read_image_dimensions(&webp_vp8x_header(900, 700)),
                d(900, 700)
            );
        }

        #[test]
        fn returns_none_for_an_unrecognized_or_truncated_header() {
            assert_eq!(read_image_dimensions(b"not an image"), None);
            assert_eq!(read_image_dimensions(&png_header(10, 10)[0..12]), None);
        }
    }

    mod to_bedrock_image_format {
        use super::*;

        #[test]
        fn maps_jpg_to_jpeg_which_is_the_name_bedrock_accepts() {
            assert_eq!(
                to_bedrock_image_format("photo.jpg"),
                Some(BedrockImageFormat::Jpeg)
            );
            assert_eq!(
                to_bedrock_image_format("photo.JPEG"),
                Some(BedrockImageFormat::Jpeg)
            );
        }

        #[test]
        fn maps_the_remaining_supported_formats() {
            assert_eq!(
                to_bedrock_image_format("a.png"),
                Some(BedrockImageFormat::Png)
            );
            assert_eq!(
                to_bedrock_image_format("a.gif"),
                Some(BedrockImageFormat::Gif)
            );
            assert_eq!(
                to_bedrock_image_format("a.webp"),
                Some(BedrockImageFormat::Webp)
            );
        }

        #[test]
        fn returns_none_for_anything_else() {
            assert_eq!(to_bedrock_image_format("a.bmp"), None);
            assert_eq!(to_bedrock_image_format("a.pdf"), None);
        }
    }

    #[test]
    fn is_image_extension_recognizes_images_case_insensitively() {
        assert!(is_image_extension("shot.PNG"));
        assert!(!is_image_extension("notes.txt"));
    }

    mod validate_image_bytes {
        use super::*;

        #[test]
        fn accepts_an_image_within_every_limit() {
            assert_eq!(
                validate_image_bytes("a.png", &png_header(100, 100), 0),
                None
            );
        }

        #[test]
        fn rejects_an_unsupported_format() {
            let message = validate_image_bytes("a.bmp", &png_header(10, 10), 0).unwrap();
            assert!(message.contains("Unsupported image format"), "{message}");
        }

        #[test]
        fn rejects_bytes_over_the_size_limit() {
            let mut big = png_header(10, 10);
            big.extend(std::iter::repeat_n(0u8, MAX_IMAGE_BYTES));
            let message = validate_image_bytes("a.png", &big, 0).unwrap();
            assert!(message.contains("3.75 MB"), "{message}");
        }

        #[test]
        fn rejects_an_image_that_is_too_many_pixels_on_a_side() {
            let message = validate_image_bytes("a.png", &png_header(9000, 100), 0).unwrap();
            assert!(message.contains("9000x100"), "{message}");
        }

        #[test]
        fn rejects_the_image_past_the_per_request_cap() {
            let message = validate_image_bytes("a.png", &png_header(10, 10), MAX_IMAGES).unwrap();
            assert!(message.contains("maximum of 20"), "{message}");
        }

        #[test]
        fn does_not_reject_an_image_whose_header_it_cannot_read() {
            assert_eq!(validate_image_bytes("a.png", b"truncated", 0), None);
        }
    }
}
