//! clrmamepro DAT text → game blocks: tokenizer and block parser (the No-Intro view
//! on top of it is `clrmamepro_dat_parser`).

use crate::clrmamepro_dat_parser::{DatGame, DatRom};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Open,
    Close,
    Text(String),
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
