//! The serial index of a [`DatIndex`]: disc DATs match a disc's serial (exact text,
//! then the system's normalised keys, see `serial_match_keys`); several entries may
//! share one serial (revisions, regional twins), the identity track size breaks ties.

use system_catalog::DiscSignature;

use crate::dat_index::DatIndex;
use crate::serial_match_keys::serial_keys;
use crate::DatEntry;

impl DatIndex {
    /// Re-indexes serials under `rule`'s keys too (a disc system's DAT).
    pub fn with_serial_rule(mut self, rule: Option<DiscSignature>) -> Self {
        self.serial_rule = rule;
        self.index_serials();
        self
    }

    pub(crate) fn index_serials(&mut self) {
        self.by_serial.clear();
        for (i, entry) in self.entries.iter().enumerate() {
            for key in entry
                .serial
                .iter()
                .flat_map(|s| serial_keys(self.serial_rule, s))
            {
                let slot = self.by_serial.entry(key).or_default();
                if slot.last() != Some(&i) {
                    slot.push(i);
                }
            }
        }
    }

    /// Entry with this serial (exact text first, then the serial rule's keys): when
    /// several share it, the one whose ROM size equals `track_size`, else the first
    /// in DAT order.
    pub fn lookup_serial(&self, serial: &str, track_size: Option<u64>) -> Option<&DatEntry> {
        let candidates = serial_keys(self.serial_rule, serial)
            .iter()
            .find_map(|key| self.by_serial.get(key))?;
        let sized =
            track_size.and_then(|size| candidates.iter().find(|&&i| self.entries[i].size == size));
        sized.or(candidates.first()).map(|&i| &self.entries[i])
    }
}
