//! Desync checksum handed to ggrs with every saved state: crc32 over the core's
//! `system_ram()` and SRAM. Serialized states are not byte-stable for most cores,
//! so the state blob is never hashed (spike 00: RAM + SRAM is exact under rollback;
//! the presented video frame is not).

/// crc32 over `system_ram` then `sram`, widened to ggrs' `u128` checksum.
pub fn desync_checksum(system_ram: &[u8], sram: &[u8]) -> u128 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(system_ram);
    hasher.update(sram);
    u128::from(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_sensitive_to_one_byte() {
        let ram = vec![7u8; 128 * 1024];
        let sram = vec![1u8; 8192];
        assert_eq!(desync_checksum(&ram, &sram), desync_checksum(&ram, &sram));
        let mut changed = sram.clone();
        changed[4000] ^= 0x10;
        assert_ne!(
            desync_checksum(&ram, &sram),
            desync_checksum(&ram, &changed)
        );
        let mut ram2 = ram.clone();
        ram2[0] = 8;
        assert_ne!(desync_checksum(&ram, &sram), desync_checksum(&ram2, &sram));
        assert!(desync_checksum(&ram, &sram) <= u128::from(u32::MAX));
    }
}
