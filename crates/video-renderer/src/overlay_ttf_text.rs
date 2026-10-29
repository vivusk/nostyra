//! Unicode overlay text (M15 achievement overlay): bundled Noto Sans drawn with
//! fontdue into an [`OverlayCanvas`], alpha-blended so it reads over translucent
//! boxes. Text is NFC-normalised first: fontdue has no mark positioning, so a
//! decomposed "ế" would stack its accents wrongly. Characters outside Latin / Greek /
//! Cyrillic (e.g. CJK) draw as the font's box. Digits are tabular in Noto Sans, so
//! changing tracker numbers keep their width.

use unicode_normalization::UnicodeNormalization;

use crate::overlay_canvas::OverlayCanvas;
pub use crate::overlay_glyph_cache::FontFace;
use crate::overlay_glyph_cache::{line_metrics, GlyphCache};

const ELLIPSIS: char = '…';

/// Text renderer with its own glyph cache (fonts are shared, parsed once).
#[derive(Default)]
pub struct TtfText {
    cache: GlyphCache,
}

/// Pixel run of a line: where each glyph lands relative to the pen start.
struct Placed {
    left: i32,
    c: char,
}

impl TtfText {
    /// `(ascent, line height)` in whole pixels at `px` (top of a line to its baseline,
    /// and the distance between two lines).
    pub fn line_metrics(&self, px: u32) -> (u32, u32) {
        match line_metrics(px) {
            Some(m) => (m.ascent.ceil() as u32, (m.ascent - m.descent).ceil() as u32),
            None => (px, px + px / 3),
        }
    }

    /// Width of `text` in pixels at `px` (kerning included).
    pub fn measure(&mut self, face: FontFace, px: u32, text: &str) -> u32 {
        self.layout(face, px, &nfc(text)).1
    }

    /// Draws `text` with its line top at `(x, y)` in `rgba`; returns its width.
    pub fn draw(
        &mut self,
        canvas: &mut OverlayCanvas,
        (x, y): (i32, i32),
        (face, px): (FontFace, u32),
        text: &str,
        rgba: [u8; 4],
    ) -> u32 {
        let (placed, width) = self.layout(face, px, &nfc(text));
        let baseline = y + self.line_metrics(px).0 as i32;
        for Placed { left, c } in placed {
            if let Some(glyph) = self.cache.glyph(face, px, c) {
                let m = &glyph.metrics;
                let top = baseline - m.height as i32 - m.ymin;
                canvas.blend_mask((x + left, top), &glyph.coverage, m.width, rgba);
            }
        }
        width
    }

    /// `text` cut with "…" so it fits `max_width` (unchanged when it already fits).
    pub fn ellipsize(&mut self, face: FontFace, px: u32, text: &str, max_width: u32) -> String {
        let text = nfc(text);
        if self.layout(face, px, &text).1 <= max_width {
            return text;
        }
        let chars: Vec<char> = text.chars().collect();
        let cut = |kept: usize| -> String {
            let prefix: String = chars[..kept].iter().collect();
            format!("{}{ELLIPSIS}", prefix.trim_end())
        };
        // Longest prefix whose "prefix…" fits (binary search: width grows with length).
        let (mut fits, mut too_long) = (0, chars.len());
        while fits + 1 < too_long {
            let mid = (fits + too_long) / 2;
            if self.layout(face, px, &cut(mid)).1 <= max_width {
                fits = mid;
            } else {
                too_long = mid;
            }
        }
        let best = cut(fits);
        if self.layout(face, px, &best).1 <= max_width {
            best
        } else {
            String::new()
        }
    }

    /// Greedy word wrap into at most `max_lines` lines of `max_width`; the last line
    /// carries the rest of the text, ellipsized. Over-long words are ellipsized.
    pub fn wrap(
        &mut self,
        face: FontFace,
        px: u32,
        text: &str,
        max_width: u32,
        max_lines: usize,
    ) -> Vec<String> {
        let text = nfc(text);
        let words: Vec<&str> = text.split_whitespace().collect();
        let mut lines = Vec::new();
        let mut index = 0;
        while index < words.len() && lines.len() < max_lines {
            if lines.len() + 1 == max_lines {
                let rest = words[index..].join(" ");
                lines.push(self.ellipsize(face, px, &rest, max_width));
                break;
            }
            let mut line = words[index].to_owned();
            index += 1;
            while index < words.len() {
                let candidate = format!("{line} {}", words[index]);
                if self.layout(face, px, &candidate).1 > max_width {
                    break;
                }
                line = candidate;
                index += 1;
            }
            lines.push(self.ellipsize(face, px, &line, max_width));
        }
        lines
    }

    /// Glyph positions (pen-relative, kerned) and the total advance of `text`.
    fn layout(&mut self, face: FontFace, px: u32, text: &str) -> (Vec<Placed>, u32) {
        let mut placed = Vec::with_capacity(text.len());
        let mut pen = 0.0f32;
        let mut previous = None;
        for c in text.chars() {
            if let Some(left) = previous {
                pen += self.cache.kern(face, px, left, c);
            }
            let Some(glyph) = self.cache.glyph(face, px, c) else {
                return (Vec::new(), 0);
            };
            placed.push(Placed {
                left: pen.round() as i32 + glyph.metrics.xmin,
                c,
            });
            pen += glyph.metrics.advance_width;
            previous = Some(c);
        }
        (placed, pen.ceil().max(0.0) as u32)
    }

    /// Glyphs rasterized so far (tests: the cache keys include the face).
    pub fn cached_glyphs(&self) -> usize {
        self.cache.len()
    }
}

fn nfc(text: &str) -> String {
    text.nfc().collect()
}

#[cfg(test)]
#[path = "overlay_ttf_text_tests.rs"]
mod tests;
