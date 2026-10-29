//! Multi-disc control on a running session, through the disk control table the core
//! registered (EXT or legacy). A swap is the libretro sequence: open the tray, select
//! the image, close the tray (the core reads the new disc a moment later). Save
//! states store the disc index themselves, so loading one needs no call here.

use std::ffi::{c_char, CStr};
use std::path::Path;

use crate::core_session::CoreSession;
use crate::ffi_disc_message_types::RetroDiskControlExtCallback;
use crate::host_error::{HostError, HostResult};

/// Buffer for image labels / paths returned by the core.
const TEXT_LEN: usize = 1024;

/// Disc access borrowed from a session (no frame runs while it lives); the disc
/// operations live on [`DiscTable`].
pub struct DiscControl<'a> {
    table: DiscTable,
    _session: &'a mut CoreSession,
}

impl std::ops::Deref for DiscControl<'_> {
    type Target = DiscTable;
    fn deref(&self) -> &DiscTable {
        &self.table
    }
}

/// The core's disk control callbacks.
pub struct DiscTable(pub(crate) RetroDiskControlExtCallback);

impl CoreSession {
    /// The core's disk control, when it registered one.
    pub fn discs(&mut self) -> Option<DiscControl<'_>> {
        let table = DiscTable(self.ctx().disk_control?);
        Some(DiscControl {
            table,
            _session: self,
        })
    }

    /// On-screen messages the core sent since the last call (oldest first).
    pub fn take_core_messages(&mut self) -> Vec<String> {
        std::mem::take(&mut self.ctx_mut().core_messages)
    }
}

impl DiscTable {
    /// Number of disc images (0 when the core cannot tell).
    pub fn count(&self) -> u32 {
        // SAFETY: core-provided callback; no host context borrow is live.
        self.0.get_num_images.map_or(0, |f| unsafe { f() })
    }

    /// Index of the inserted disc.
    pub fn index(&self) -> u32 {
        // SAFETY: as `count`.
        self.0.get_image_index.map_or(0, |f| unsafe { f() })
    }

    /// Short name of disc `index`: the core's label reduced to its file-name part
    /// (PCSX ReARMed labels with the full path minus extension), else the image path's
    /// file stem; "Disc N" when the core has neither.
    pub fn label(&self, index: u32) -> String {
        let label = self.text(self.0.get_image_label, index).map(|t| {
            let t = t.trim().to_owned();
            Path::new(&t)
                .file_name()
                .map_or(t.clone(), |n| n.to_string_lossy().into_owned())
        });
        let path = || {
            let t = self.text(self.0.get_image_path, index)?;
            Path::new(t.trim())
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
        };
        label
            .or_else(path)
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("Disc {}", index + 1))
    }

    /// Inserts disc `index`: eject → select → close. Fails when the core refuses a step.
    pub fn swap_to(&self, index: u32) -> HostResult<()> {
        let (Some(eject), Some(select)) = (self.0.set_eject_state, self.0.set_image_index) else {
            return Err(HostError::DiscSwapRejected(index));
        };
        let previous = self.index();
        // SAFETY: core-provided callbacks, called in the documented order.
        unsafe {
            if !eject(true) {
                return Err(HostError::DiscSwapRejected(index));
            }
            let selected = select(index);
            if !selected {
                // Never leave the tray open: put the previous disc back.
                select(previous);
            }
            let closed = eject(false);
            if selected && closed {
                Ok(())
            } else {
                Err(HostError::DiscSwapRejected(index))
            }
        }
    }

    fn text(
        &self,
        getter: Option<crate::ffi_disc_message_types::GetImageTextFn>,
        index: u32,
    ) -> Option<String> {
        let getter = getter?;
        let mut buf = [0 as c_char; TEXT_LEN];
        // SAFETY: core writes at most `TEXT_LEN` bytes (NUL included) into `buf`.
        if !unsafe { getter(index, buf.as_mut_ptr(), TEXT_LEN) } {
            return None;
        }
        buf[TEXT_LEN - 1] = 0;
        // SAFETY: NUL-terminated (forced above).
        let text = unsafe { CStr::from_ptr(buf.as_ptr()) }
            .to_string_lossy()
            .into_owned();
        (!text.trim().is_empty()).then_some(text)
    }
}

#[cfg(test)]
#[path = "core_session_disc_control_tests.rs"]
mod tests;
