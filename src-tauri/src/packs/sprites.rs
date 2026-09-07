//! Sprite strip decoding. The engine needs three things from a PNG: its real
//! dimensions (to check them against the manifest), a per-frame alpha mask (so
//! clicks pass through empty pixels), and the bytes to hand the renderer.
use super::diagnostics::{Diagnostic, Report};
use std::path::Path;

/// One decoded animation strip.
#[derive(Clone, Debug)]
pub struct Strip {
    pub width: u32,
    pub height: u32,
    pub frame_width: u32,
    /// One mask per frame; `true` where the pixel is opaque enough to click.
    pub masks: Vec<Mask>,
    /// The deepest row any frame of this clip reaches — its contact point.
    /// Frames export with transparent padding below this, and airborne
    /// frames (hops, strides) sit above it; only the padding must be
    /// skipped when anchoring, or the pet floats above its surface.
    pub ground: u32,
    /// `data:image/png;base64,...` — the webview has no filesystem access.
    pub data_uri: String,
}

#[derive(Clone, Debug)]
pub struct Mask {
    pub width: u32,
    pub height: u32,
    /// Rows of visible pixels, counting from the top: `bottom` is one past
    /// the lowest opaque row of this frame.
    pub bottom: u32,
    bits: Vec<u8>,
}
impl Mask {
    pub fn opaque(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let i = (y * self.width + x) as usize;
        self.bits
            .get(i / 8)
            .is_some_and(|b| b & (1 << (i % 8)) != 0)
    }
}

const OPACITY_THRESHOLD: u8 = 24;

pub fn load(path: &Path, relative: &str, frames: u32, report: &mut Report) -> Option<Strip> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            report.push(Diagnostic::error(
                relative,
                "",
                format!("The sprite file could not be read: {error}."),
                "Check that the file exists and the name matches the manifest exactly, including capitals.",
            ));
            return None;
        }
    };
    let decoder = png::Decoder::new(std::io::Cursor::new(&bytes));
    let mut reader = match decoder.read_info() {
        Ok(reader) => reader,
        Err(error) => {
            report.push(Diagnostic::error(
                relative,
                "",
                format!("The file is not a readable PNG: {error}."),
                "Export the strip as a PNG with transparency.",
            ));
            return None;
        }
    };
    let mut buffer = vec![0; reader.output_buffer_size()];
    let info = match reader.next_frame(&mut buffer) {
        Ok(info) => info,
        Err(error) => {
            report.push(Diagnostic::error(
                relative,
                "",
                format!("The PNG pixels could not be decoded: {error}."),
                "Re-export the image; it may be truncated.",
            ));
            return None;
        }
    };
    let (width, height) = (info.width, info.height);
    let channels = info.color_type.samples();
    let bytes_per_sample = (info.bit_depth as usize).div_euclid(8).max(1);
    let alpha_index = match info.color_type {
        png::ColorType::Rgba => Some(3),
        png::ColorType::GrayscaleAlpha => Some(1),
        _ => None,
    };
    if alpha_index.is_none() {
        report.push(Diagnostic::info(
            relative,
            "",
            "The image has no alpha channel.",
            "The pet will be a solid rectangle. Export with transparency to shape it.",
        ));
    }
    let frames = frames.max(1);
    if width % frames != 0 {
        report.push(Diagnostic::warning(
            relative,
            "",
            format!(
                "The image is {width} px wide, which does not divide evenly into {frames} frames."
            ),
            format!(
                "Each frame would be {:.2} px. Make the width a multiple of the frame count.",
                width as f64 / frames as f64
            ),
        ));
    }
    let frame_width = (width / frames).max(1);
    let mut masks = Vec::with_capacity(frames as usize);
    for frame in 0..frames {
        let mut bits = vec![0_u8; ((frame_width * height) as usize).div_ceil(8)];
        let mut bottom = 0;
        for y in 0..height {
            for x in 0..frame_width {
                let source_x = frame * frame_width + x;
                if source_x >= width {
                    continue;
                }
                let pixel = ((y * width + source_x) as usize) * channels * bytes_per_sample;
                let opaque = match alpha_index {
                    None => true,
                    Some(offset) => buffer
                        .get(pixel + offset * bytes_per_sample)
                        .is_some_and(|&a| a >= OPACITY_THRESHOLD),
                };
                if opaque {
                    let i = (y * frame_width + x) as usize;
                    bits[i / 8] |= 1 << (i % 8);
                    bottom = y + 1;
                }
            }
        }
        masks.push(Mask {
            width: frame_width,
            height,
            bottom,
            bits,
        });
    }
    let ground = masks.iter().map(|m| m.bottom).max().unwrap_or(0);
    Some(Strip {
        width,
        height,
        frame_width,
        masks,
        ground,
        data_uri: format!("data:image/png;base64,{}", base64(&bytes)),
    })
}

/// Small standard base64 encoder; the alternative is a dependency for 20 lines.
pub fn base64(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(ALPHABET[(n >> 18) as usize & 63] as char);
        out.push(ALPHABET[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            ALPHABET[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            ALPHABET[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn base64_matches_the_standard_alphabet_and_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }
}
