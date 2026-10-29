//! libretro core host: dynamic loading, hand-written FFI and callback bridging.
//!
//! The only crate allowed to contain `unsafe` (FFI boundary). Consumers drive a
//! [`CoreSession`] frame by frame and receive output through the
//! [`VideoSink`] / [`AudioSink`] / [`InputSource`] traits.

mod av_callbacks;
mod core_library_loader;
mod core_probe;
mod core_session;
mod core_session_core_rules;
mod core_session_disc_control;
mod core_session_game_loading;
mod core_session_hw_render;
mod core_session_memory_access;
mod core_session_port_devices;
mod core_session_save_data;
mod core_shutdown_request;
mod environment_av_info;
mod environment_callback;
mod environment_core_messages;
mod environment_core_options;
mod environment_disk_control;
mod environment_hw_render;
mod environment_memory_maps;
mod ffi_disc_message_types;
mod ffi_hw_render_types;
mod ffi_types;
mod frame_sink_traits;
mod host_context;
mod host_error;
mod hw_render;
#[cfg(test)]
mod hw_render_test_fakes;
mod memory_map_snapshot;
mod process_environment;
mod process_session_claim;
mod savestate_context;
mod serialization_quirks;
mod session_config;
mod staged_content_file;

pub use core_library_loader::CoreInfo;
pub use core_probe::probe_core;
pub use core_session::{AvInfo, CoreSession, SessionConfig};
pub use core_session_disc_control::{DiscControl, DiscTable};
pub use ffi_types::{
    RetroGameGeometry, RetroMemoryDescriptor, RetroSystemTiming, RETRO_DEVICE_ANALOG,
    RETRO_DEVICE_ID_POINTER_COUNT, RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN,
    RETRO_DEVICE_ID_POINTER_PRESSED, RETRO_DEVICE_ID_POINTER_X, RETRO_DEVICE_ID_POINTER_Y,
    RETRO_DEVICE_JOYPAD, RETRO_DEVICE_NONE, RETRO_DEVICE_POINTER, RETRO_MEMORY_SAVE_RAM,
    RETRO_MEMORY_SYSTEM_RAM,
};
pub use frame_sink_traits::{
    AudioSink, InputSource, NullAudioSink, NullInputSource, NullVideoSink, PixelFormat, VideoFrame,
    VideoSink,
};
pub use host_context::AvEnable;
pub use host_error::{HostError, HostResult};
pub use hw_render::{HwContextKind, HwRenderContext, HwRenderProvider, HwRenderRequest};
pub use memory_map_snapshot::MemoryMapSnapshot;
pub use process_environment::set_process_env;
pub use savestate_context::SavestateContext;
pub use serialization_quirks::SerializationQuirks;
