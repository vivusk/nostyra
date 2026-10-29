//! [`CoreSpec`] and its value types: save-state quality, BIOS files, memory card
//! slots, core system assets; [`CORE_DEFAULTS`] for the fields most cores leave off.

/// Save-state quality of a core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStateQuality {
    /// A state captures the exact frame: run-ahead and rewind may use it.
    Deterministic,
    /// Slots and resume work, but serializing advances the emulation to a safe point
    /// (and may poll input), so run-ahead and rewind are refused.
    Serialized,
}

/// A BIOS file a core can use, identified by md5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FirmwareFile {
    /// Canonical lower-case file name (`scph1001.bin`).
    pub name: &'static str,
    pub md5: &'static str,
    /// Region it boots ("US", "JP", "EU").
    pub region: &'static str,
    /// Console model ("SCPH-1001").
    pub model: &'static str,
    /// Exact file size in bytes.
    pub size: u64,
    /// Folder under `<data>/system/` the core looks in (`""` = the system folder
    /// itself, `"dc"` for Flycast).
    pub subdir: &'static str,
}

/// A core option that picks the real BIOS or the core's HLE BIOS. Cores that boot a
/// present BIOS file without checking it (Flycast) get it answered by the runner: the
/// real BIOS only when a known file (name, size, md5) is there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BiosOptionSwitch {
    pub key: &'static str,
    /// Value that lets the core boot the real BIOS file.
    pub real: &'static str,
    /// Value that forces the built-in HLE BIOS.
    pub hle: &'static str,
}

/// Where one memory card slot of a core lives in the game's save folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryCardSlot {
    /// The libretro `SAVE_RAM` buffer (persisted by the runner as the save file).
    SaveRam,
    /// A file the core reads and writes itself in the save directory.
    CoreFile(&'static str),
}

/// Files a core needs in `<data>/system/<dir>/` (fonts, dialogs, compat tables), shipped
/// by the libretro buildbot as a separate archive. Downloaded with the core into
/// `<cores>/<dir>/` and installed into the system folder by the runner before load.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreAssets {
    /// Folder name under the system folder and beside the core library (`PPSSPP`).
    pub dir: &'static str,
    /// Files that must exist for the assets to count as installed; the first one is
    /// also compared by content to pick up a newer copy beside the core.
    pub markers: &'static [&'static str],
    /// Archive path under the buildbot root (`assets/system/PPSSPP.zip`).
    pub archive_path: &'static str,
}

/// Save-state size cap of most cores (largest state the runner reads or writes).
pub const DEFAULT_MAX_STATE_BYTES: u64 = 64 * 1024 * 1024;

/// One downloadable libretro core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CoreSpec {
    /// Nostyra core id (`cores.id`); equals `core_layer_id(library_name)` of the
    /// core, so `config/core/<id>.toml` is its config layer.
    pub id: &'static str,
    /// Library file name without the OS extension (`snes9x_libretro`).
    pub library_stem: &'static str,
    /// Display name for Settings → Cores and install prompts.
    pub display_name: &'static str,
    /// System ids this core plays.
    pub systems: &'static [&'static str],
    pub license: &'static str,
    /// Core options the host always answers (`GET_VARIABLE`), as `(key, value)`;
    /// every other key gets the core's default.
    pub fixed_options: &'static [(&'static str, &'static str)],
    /// Optional BIOS files the core looks for in `<data>/system/` (never bundled).
    pub firmware: &'static [FirmwareFile],
    /// Option the runner answers with real/HLE BIOS from `firmware` (see
    /// [`BiosOptionSwitch`]); `None` when the core checks its BIOS itself.
    pub bios_switch: Option<BiosOptionSwitch>,
    /// Memory cards the core keeps per game (empty for cartridge cores).
    pub memory_cards: &'static [MemoryCardSlot],
    /// How exact the core's save states are (the libretro `.info` `savestate` field).
    pub savestates: SaveStateQuality,
    /// `RETRO_DEVICE_*` per port, set with `retro_set_controller_port_device` after
    /// load. Empty: never called (the core keeps its own default devices).
    pub port_devices: &'static [u32],
    /// Refuse save states whose length differs from the core's state size before
    /// they reach the core (Mupen64Plus-Next hangs on them).
    pub exact_state_size: bool,
    /// Never cache the state size: the core's `retro_serialize_size` changes without
    /// the `CORE_VARIABLE_SIZE` quirk (PPSSPP: 128 MiB before boot, then measured).
    /// The runner also holds a startup resume until the core has booted.
    pub requery_state_size: bool,
    /// Largest save state the runner writes or loads for this core.
    pub max_state_bytes: u64,
    /// System files the core needs beside its BIOS (`None`: nothing to install).
    pub system_assets: Option<CoreAssets>,
    /// Environment variables the runner sets in its own process before loading the
    /// core (PPSSPP: `CI`, so a release assert never opens a modal dialog).
    pub process_env: &'static [(&'static str, &'static str)],
    /// A loaded core without a hardware render context is fatal (exit 6): the core
    /// would silently fall back to a black Null renderer (Dolphin). Enforced by the
    /// runner from M14 phase 03.
    pub hw_render_required: bool,
    /// Refuse save states made by another core version instead of warning (the core
    /// accepts any blob; Dolphin). Enforced by the runner from M14 phase 03.
    pub strict_state_version: bool,
    /// Never unload the core library at session end: Dolphin hangs the session
    /// thread's exit after a `retro_unserialize` + unload (M14 spike). Enforced by
    /// the runner from M14 phase 03.
    pub keep_library_loaded: bool,
    /// Options RetroAchievements hardcore requires at exactly these values, on top of
    /// rcheevos' own disallowed-option table (which matches `library_name`; melonDS DS
    /// reports "melonDS DS", not rcheevos' "melonDS", so its DSi rule is restated here).
    pub hardcore_required_options: &'static [(&'static str, &'static str)],
}

/// Defaults for the fields most cores do not use; registry entries end with
/// `..CORE_DEFAULTS` after their own fields.
pub const CORE_DEFAULTS: CoreSpec = CoreSpec {
    id: "",
    library_stem: "",
    display_name: "",
    systems: &[],
    license: "",
    fixed_options: &[],
    firmware: &[],
    bios_switch: None,
    memory_cards: &[],
    savestates: SaveStateQuality::Deterministic,
    port_devices: &[],
    exact_state_size: false,
    requery_state_size: false,
    max_state_bytes: DEFAULT_MAX_STATE_BYTES,
    system_assets: None,
    process_env: &[],
    hw_render_required: false,
    strict_state_version: false,
    keep_library_loaded: false,
    hardcore_required_options: &[],
};
