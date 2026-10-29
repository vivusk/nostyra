//! Bundled overlay fonts and their glyph cache. Noto Sans Regular + Bold (OFL-1.1,
//! `assets/fonts/`, see `SOURCE.txt` there) are embedded in the binary but parsed only
//! on the first text call (parsing costs 30–150 ms per face, M15 spike D), once per
//! process. Rasterized glyphs are cached per (face, pixel size, char): one face's
//! glyph must never stand in for the other's (spike finding).

use std::collections::HashMap;
use std::sync::OnceLock;

use fontdue::{Font, FontSettings, LineMetrics, Metrics};

const REGULAR_TTF: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Regular.ttf");
const BOLD_TTF: &[u8] = include_bytes!("../../../assets/fonts/NotoSans-Bold.ttf");
/// Cache bound: a few sizes of a few hundred distinct characters fit easily; past it
/// the cache starts over (overlay redraws are rare, so a refill is cheap).
const MAX_GLYPHS: usize = 4096;

/// Which bundled face draws a run of text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FontFace {
    Regular,
    Bold,
}

struct Fonts {
    regular: Font,
    bold: Font,
}

impl Fonts {
    fn face(&self, face: FontFace) -> &Font {
        match face {
            FontFace::Regular => &self.regular,
            FontFace::Bold => &self.bold,
        }
    }
}

/// The parsed faces (`None` if the embedded files were unreadable: text then draws
/// nothing rather than failing the frame). Parsed on first use.
fn fonts() -> Option<&'static Fonts> {
    static FONTS: OnceLock<Option<Fonts>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let parse = |bytes: &[u8]| Font::from_bytes(bytes, FontSettings::default());
            match (parse(REGULAR_TTF), parse(BOLD_TTF)) {
                (Ok(regular), Ok(bold)) => Some(Fonts { regular, bold }),
                (Err(err), _) | (_, Err(err)) => {
                    tracing::error!(err, "bundled overlay font unreadable; overlay text off");
                    None
                }
            }
        })
        .as_ref()
}

/// One rasterized glyph: metrics + 8-bit coverage rows (`metrics.width` columns).
/// A character the font lacks rasterizes as the font's own `.notdef` box.
pub(crate) struct Glyph {
    pub(crate) metrics: Metrics,
    pub(crate) coverage: Vec<u8>,
}

/// Glyphs keyed by (face, px, char).
#[derive(Default)]
pub(crate) struct GlyphCache {
    glyphs: HashMap<(FontFace, u32, char), Glyph>,
}

impl GlyphCache {
    /// The glyph of `c` (rasterized on first use); `None` without fonts.
    pub(crate) fn glyph(&mut self, face: FontFace, px: u32, c: char) -> Option<&Glyph> {
        let font = fonts()?.face(face);
        let key = (face, px, c);
        if self.glyphs.len() >= MAX_GLYPHS && !self.glyphs.contains_key(&key) {
            self.glyphs.clear();
        }
        Some(self.glyphs.entry(key).or_insert_with(|| {
            let (metrics, coverage) = font.rasterize(c, px as f32);
            Glyph { metrics, coverage }
        }))
    }

    /// Kerning between two characters of `face` at `px` (0 without a pair).
    pub(crate) fn kern(&self, face: FontFace, px: u32, left: char, right: char) -> f32 {
        fonts()
            .and_then(|fonts| fonts.face(face).horizontal_kern(left, right, px as f32))
            .unwrap_or(0.0)
    }

    pub(crate) fn len(&self) -> usize {
        self.glyphs.len()
    }
}

/// Line metrics of the Regular face at `px` (both faces share them).
pub(crate) fn line_metrics(px: u32) -> Option<LineMetrics> {
    fonts()?.regular.horizontal_line_metrics(px as f32)
}
