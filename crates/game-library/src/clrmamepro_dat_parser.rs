//! Minimal clrmamepro DAT parser for the libretro-database `metadat/` formats:
//!
//! ```text
//! game (                                   game (                       (metadat/genre)
//!     name "Super Mario World (USA)"           comment "Super Mario World (USA)"
//!     region "USA"                             genre "Platform"
//!     rom ( name "…sfc" size 524288            rom ( crc B19ED489 )
//!           crc B19ED489 sha1 6B47BB75… )   )
//! )
//! ```
//! [`parse_clrmamepro_games`] keeps every scalar `key value` of a game block (so the
//! attribute DATs need no special parser); [`parse_clrmamepro_dat`] is the No-Intro
//! view used for identification. Nested blocks other than `rom` and the `clrmamepro`
//! header block are skipped.

use std::collections::HashMap;

/// One ROM of a DAT game. Attribute DATs carry only the CRC (Nintendo) or only a
/// serial + name (PlayStation metadat: no CRC at all).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatRom {
    /// ROM file name (`"Game (USA).nes"`), when present.
    pub name: Option<String>,
    pub size: Option<u64>,
    /// Upper-case hex, when present.
    pub crc32: Option<String>,
    /// Upper-case hex, when present.
    pub sha1: Option<String>,
    /// ROM-level `serial` (PSP `releaseyear` metadat keys games only here).
    pub serial: Option<String>,
}

/// One `game ( … )` block: scalar keys in file order plus its ROMs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatGame {
    /// Scalar `key value` pairs (`name`, `comment`, `region`, `genre`, `users`…).
    pub attrs: Vec<(String, String)>,
    pub roms: Vec<DatRom>,
}

impl DatGame {
    /// First value of `key`, if present.
    pub fn attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// One ROM of a No-Intro DAT game (a multi-ROM game yields several entries).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatEntry {
    /// Full No-Intro game name, tags included.
    pub name: String,
    pub region: Option<String>,
    pub size: u64,
    /// Upper-case hex.
    pub crc32: String,
    /// Upper-case hex; a few DAT entries have none.
    pub sha1: Option<String>,
    /// Game-level `serial` (Redump disc DATs: `SLUS-01234`); `None` for No-Intro.
    pub serial: Option<String>,
    /// CRC the metadat attribute files key this game by: the headered `.nes` ROM's
    /// CRC for NES (also for its headerless `.unh` entry), the entry's own CRC
    /// otherwise.
    pub metadata_crc32: String,
}

/// ROM file extension of headered NES dumps; metadat files key NES games by its CRC.
const HEADERED_NES_EXTENSION: &str = ".nes";

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Text(String),
}

/// Parses No-Intro DAT text into ROM entries: every game needs a `name`, every ROM a
/// `size` and `crc`. Errors carry a short description.
///
/// The libretro NES DAT lists the headered `.nes` ROM and the headerless `.unh` ROM
/// of a game in two separate `game` blocks with the same name; after parsing, each
/// headerless entry gets its `.nes` twin's CRC as `metadata_crc32` (own CRC when no
/// single twin is identifiable, e.g. every SNES game).
pub fn parse_clrmamepro_dat(text: &str) -> Result<Vec<DatEntry>, String> {
    let mut entries = Vec::new();
    let mut headered: HashMap<String, Vec<(u64, String)>> = HashMap::new();
    let mut is_headered = Vec::new();
    for game in parse_clrmamepro_games(text)? {
        let name = game.attr("name").ok_or("game without name")?.to_string();
        let region = game.attr("region").map(str::to_string);
        let serial = game.attr("serial").map(str::to_string);
        for rom in game.roms {
            let size = rom.size.ok_or("rom without size or crc")?;
            let crc32 = rom.crc32.ok_or("rom without size or crc")?;
            let nes = rom
                .name
                .as_deref()
                .is_some_and(|file| file.to_ascii_lowercase().ends_with(HEADERED_NES_EXTENSION));
            if nes {
                headered
                    .entry(name.clone())
                    .or_default()
                    .push((size, crc32.clone()));
            }
            is_headered.push(nes);
            entries.push(DatEntry {
                name: name.clone(),
                region: region.clone(),
                size,
                metadata_crc32: crc32.clone(),
                crc32,
                sha1: rom.sha1,
                serial: serial.clone(),
            });
        }
    }
    // A headerless entry takes the CRC of its headered twin; names repeat in the DAT
    // (different dumps), so pair by size and keep the own CRC when not unambiguous.
    for (entry, nes) in entries.iter_mut().zip(is_headered) {
        if nes {
            continue;
        }
        if let Some(crc) = headered
            .get(&entry.name)
            .and_then(|twins| headerless_twin(twins, entry.size))
        {
            entry.metadata_crc32.clone_from(crc);
        }
    }
    Ok(entries)
}

/// Size of the iNES header, and of the optional trainer a headerless dump also drops.
const INES_HEADER_SIZE: u64 = 16;
const INES_TRAINER_SIZE: u64 = 512;

/// CRC of the single headered ROM that `size` (a headerless dump) derives from; the
/// only twin when the name has one, else the one whose size fits.
fn headerless_twin(twins: &[(u64, String)], size: u64) -> Option<&String> {
    if let [(_, crc)] = twins {
        return Some(crc);
    }
    let mut fits = twins.iter().filter(|(nes_size, _)| {
        *nes_size == size + INES_HEADER_SIZE
            || *nes_size == size + INES_HEADER_SIZE + INES_TRAINER_SIZE
    });
    match (fits.next(), fits.next()) {
        (Some((_, crc)), None) => Some(crc),
        _ => None,
    }
}

/// Parses DAT text into game blocks (any clrmamepro DAT, attribute DATs included).
pub fn parse_clrmamepro_games(text: &str) -> Result<Vec<DatGame>, String> {
    let tokens = tokenize(text)?;
    let mut games = Vec::new();
    let mut i = 0;
    while i < tokens.len() {
        match (&tokens[i], tokens.get(i + 1)) {
            (Token::Text(word), Some(Token::Open)) => {
                let end = block_end(&tokens, i + 1)?;
                if word == "game" {
                    games.push(parse_game(&tokens[i + 2..end])?);
                }
                i = end + 1;
            }
            _ => return Err(format!("unexpected token {:?} at top level", tokens[i])),
        }
    }
    Ok(games)
}

/// Splits into parentheses, quoted strings (no escapes in libretro DATs) and bare words.
fn tokenize(text: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        match ch {
            c if c.is_whitespace() => {}
            '(' => tokens.push(Token::Open),
            ')' => tokens.push(Token::Close),
            '"' => {
                let rest = &text[start + 1..];
                let len = rest.find('"').ok_or("unterminated quoted string")?;
                tokens.push(Token::Text(rest[..len].to_string()));
                // Skip the string body and the closing quote.
                while chars.next_if(|&(pos, _)| pos <= start + 1 + len).is_some() {}
            }
            _ => {
                let mut end = start + ch.len_utf8();
                while let Some(&(pos, c)) = chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' || c == '"' {
                        break;
                    }
                    end = pos + c.len_utf8();
                    chars.next();
                }
                tokens.push(Token::Text(text[start..end].to_string()));
            }
        }
    }
    Ok(tokens)
}

/// Index of the `)` matching the `(` at `open`.
fn block_end(tokens: &[Token], open: usize) -> Result<usize, String> {
    let mut depth = 0usize;
    for (offset, token) in tokens[open..].iter().enumerate() {
        match token {
            Token::Open => depth += 1,
            Token::Close => {
                depth -= 1;
                if depth == 0 {
                    return Ok(open + offset);
                }
            }
            Token::Text(_) => {}
        }
    }
    Err("unbalanced parentheses".into())
}

fn parse_game(body: &[Token]) -> Result<DatGame, String> {
    let mut game = DatGame {
        attrs: Vec::new(),
        roms: Vec::new(),
    };
    let mut i = 0;
    while i < body.len() {
        let Token::Text(key) = &body[i] else {
            return Err(format!("unexpected {:?} in game block", body[i]));
        };
        if body.get(i + 1) == Some(&Token::Open) {
            let end = block_end(body, i + 1)?;
            if key == "rom" {
                game.roms.push(parse_rom(&body[i + 2..end])?);
            }
            i = end + 1;
            continue;
        }
        match body.get(i + 1) {
            Some(Token::Text(value)) => game.attrs.push((key.clone(), value.clone())),
            other => return Err(format!("key {key} without value: {other:?}")),
        }
        i += 2;
    }
    Ok(game)
}

fn parse_rom(body: &[Token]) -> Result<DatRom, String> {
    let (mut name, mut size, mut crc32, mut sha1, mut serial) = (None, None, None, None, None);
    for pair in body.chunks(2) {
        if let [Token::Text(key), Token::Text(value)] = pair {
            match key.as_str() {
                "name" => name = Some(value.clone()),
                "size" => size = value.parse::<u64>().ok(),
                "crc" => crc32 = Some(value.to_ascii_uppercase()),
                "sha1" => sha1 = Some(value.to_ascii_uppercase()),
                "serial" => serial = Some(value.clone()),
                _ => {}
            }
        }
    }
    Ok(DatRom {
        name,
        size,
        crc32,
        sha1,
        serial,
    })
}
