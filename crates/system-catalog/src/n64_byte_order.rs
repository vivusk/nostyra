//! Nintendo 64 dump byte orders. The cartridge header starts with `80 37 12 40`; a
//! `.z64` dump keeps that big-endian order (what No-Intro hashes), a `.v64` dump
//! swaps every 16-bit pair (Doctor V64) and a `.n64` dump reverses every 32-bit word.
//! The first 4 bytes tell the order, whatever the extension says.

/// Byte order of an N64 dump, read from its first 4 bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum N64ByteOrder {
    /// `.z64`: `80 37 12 40`, the cartridge order.
    BigEndian,
    /// `.v64`: `37 80 40 12`, 16-bit pairs swapped.
    ByteSwapped,
    /// `.n64`: `40 12 37 80`, 32-bit words reversed.
    LittleEndian,
}

/// First word of a big-endian (`.z64`) N64 image.
pub(crate) const Z64_MAGIC: [u8; 4] = [0x80, 0x37, 0x12, 0x40];
/// First word of a 16-bit swapped (`.v64`) image.
pub(crate) const V64_MAGIC: [u8; 4] = [0x37, 0x80, 0x40, 0x12];
/// First word of a 32-bit little-endian (`.n64`) image.
pub(crate) const N64_MAGIC: [u8; 4] = [0x40, 0x12, 0x37, 0x80];

/// Byte order of a dump starting with `head`; `None` when no N64 magic is there.
pub(crate) fn detect(head: &[u8]) -> Option<N64ByteOrder> {
    match head.get(..4)? {
        h if h == Z64_MAGIC => Some(N64ByteOrder::BigEndian),
        h if h == V64_MAGIC => Some(N64ByteOrder::ByteSwapped),
        h if h == N64_MAGIC => Some(N64ByteOrder::LittleEndian),
        _ => None,
    }
}

/// Turns a `.v64` / `.n64` dump into the `.z64` image in place (no copy); a `.z64`
/// dump or a file without the magic is left as is. A trailing remainder shorter
/// than one 16/32-bit unit is left as is (no real dump has one).
pub fn normalise_in_place(bytes: &mut [u8]) {
    if let Some(order) = detect(bytes) {
        swap_order(bytes, order);
    }
}

/// `.z64` image → the same dump in `order` (inverse of [`normalise_in_place`]).
/// Test helper shared by every crate's N64 tests.
#[doc(hidden)]
pub fn n64_to_order(z64: &[u8], order: N64ByteOrder) -> Vec<u8> {
    let mut out = z64.to_vec();
    swap_order(&mut out, order);
    out
}

/// Swaps between `order` and big-endian. Both swaps are their own inverse, so the
/// same call converts either way.
fn swap_order(bytes: &mut [u8], order: N64ByteOrder) {
    match order {
        N64ByteOrder::BigEndian => {}
        N64ByteOrder::ByteSwapped => bytes
            .as_chunks_mut::<2>()
            .0
            .iter_mut()
            .for_each(|c| c.swap(0, 1)),
        N64ByteOrder::LittleEndian => bytes
            .as_chunks_mut::<4>()
            .0
            .iter_mut()
            .for_each(|c| c.reverse()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z64_rom() -> Vec<u8> {
        let mut rom: Vec<u8> = (0..0x2000u32).map(|i| (i * 13 % 251) as u8).collect();
        rom[..4].copy_from_slice(&Z64_MAGIC);
        rom
    }

    #[test]
    fn every_order_round_trips_to_z64() {
        let z64 = z64_rom();
        for (order, magic) in [
            (N64ByteOrder::BigEndian, Z64_MAGIC),
            (N64ByteOrder::ByteSwapped, V64_MAGIC),
            (N64ByteOrder::LittleEndian, N64_MAGIC),
        ] {
            let mut dump = n64_to_order(&z64, order);
            assert_eq!(dump[..4], magic, "{order:?}");
            assert_eq!(detect(&dump), Some(order));
            normalise_in_place(&mut dump);
            assert_eq!(dump, z64, "{order:?}");
        }
    }

    #[test]
    fn z64_junk_and_short_input_stay_untouched() {
        let z64 = z64_rom();
        let mut copy = z64.clone();
        normalise_in_place(&mut copy);
        assert_eq!(copy, z64);
        let mut junk = vec![1u8, 2, 3, 4, 5, 6];
        normalise_in_place(&mut junk);
        assert_eq!(junk, [1, 2, 3, 4, 5, 6]);
        assert_eq!(detect(&V64_MAGIC[..3]), None);
        // An odd tail after the last whole unit is kept as is.
        let mut odd = n64_to_order(&z64, N64ByteOrder::LittleEndian);
        odd.push(0xEE);
        normalise_in_place(&mut odd);
        assert_eq!(odd[..z64.len()], z64[..]);
        assert_eq!(odd.last(), Some(&0xEE));
    }
}
