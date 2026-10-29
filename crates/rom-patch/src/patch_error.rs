//! Why a patch could not be applied (the game then starts unpatched).

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PatchError {
    #[error("not an IPS, UPS or BPS patch")]
    UnknownFormat,
    #[error("patch file is truncated or corrupt")]
    Truncated,
    #[error("patch was made for a different ROM (checksum mismatch)")]
    SourceMismatch,
    #[error("patched ROM failed the patch's checksum")]
    TargetMismatch,
    #[error("patch file failed its own checksum")]
    PatchChecksum,
    #[error("patch refers to data outside the ROM")]
    OutOfRange,
}
