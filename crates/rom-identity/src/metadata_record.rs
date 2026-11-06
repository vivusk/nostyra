//! One game's offline metadata (vendored libretro-database metadat) and how a metadat
//! attribute value is stored in it.

/// Index-derived metadata of one game. The desktop converts it to the storage-db
/// record; the web edition sends it to TypeScript as JSON.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MetadataRecord {
    pub developer: Option<String>,
    pub publisher: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<u16>,
    pub players: Option<u8>,
}

/// Stores one attribute value (`folder` = metadat attribute folder); unparsable
/// numbers are dropped, blanks ignored.
pub(crate) fn apply_attribute(record: &mut MetadataRecord, folder: &str, value: &str) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    match folder {
        "developer" => record.developer = Some(value.to_string()),
        "publisher" => record.publisher = Some(value.to_string()),
        "genre" => record.genre = Some(value.to_string()),
        "releaseyear" => record.release_year = value.parse().ok(),
        "maxusers" => record.players = value.parse().ok().filter(|&n: &u8| n > 0),
        _ => {}
    }
}
