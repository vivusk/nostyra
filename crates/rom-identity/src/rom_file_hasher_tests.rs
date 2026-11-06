//! Hash vectors, header rules and the owned/borrowed entry points of `rom_file_hasher`.

use super::*;
use system_catalog::{n64_to_order, raw_to_smd, N64ByteOrder, GENESIS, NES, NINTENDO_64, SNES};

#[test]
fn known_vectors() {
    // CRC32/SHA1 of "123456789" padded to stay header-free (len % 1024 != 512).
    let hashes = hash_rom_bytes(b"123456789", &SNES);
    assert_eq!(hashes.crc32, "CBF43926");
    assert_eq!(hashes.sha1, "F7C3BC1D808E04732ADF679965CCC34CA7AE3441");
    assert_eq!(hashes.size, 9);
}

#[test]
fn streaming_hash_equals_in_memory_hash() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-hasher-")
        .tempdir()
        .unwrap();
    let path = temp.path().join("track.bin");
    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 31 % 253) as u8).collect();
    std::fs::write(&path, &data).unwrap();
    assert_eq!(hash_file_streaming(&path).unwrap(), hash_image(&data));
}

#[test]
fn file_over_the_size_limit_is_rejected_without_hashing() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-hasher-")
        .tempdir()
        .unwrap();
    let path = temp.path().join("oversize.sfc");
    std::fs::write(&path, vec![1u8; 40_000]).unwrap();
    let small = SystemSpec {
        max_bytes: 32 * 1024,
        ..SNES
    };
    let result = read_rom_file(&path, &small);
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
}

#[test]
fn headered_and_headerless_copies_hash_the_same() {
    let image: Vec<u8> = (0..65_536u32).map(|i| (i * 7 % 251) as u8).collect();
    let mut headered = vec![0xAA; 512];
    headered.extend_from_slice(&image);
    assert_eq!(
        hash_rom_bytes(&image, &SNES),
        hash_rom_bytes(&headered, &SNES)
    );
    assert_eq!(hash_headerless(&headered, &SNES), None);
}

#[test]
fn reheadered_nes_differs_whole_but_matches_headerless() {
    let body: Vec<u8> = (0..24_576u32).map(|i| (i * 13 % 251) as u8).collect();
    let mut ines1 = vec![b'N', b'E', b'S', 0x1A, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    ines1.extend_from_slice(&body);
    let mut nes2 = ines1.clone();
    nes2[7] = 0x08; // NES 2.0 marker: same game, rewritten header
    assert_ne!(hash_rom_bytes(&ines1, &NES), hash_rom_bytes(&nes2, &NES));
    assert_eq!(hash_rom_bytes(&ines1, &NES).size, ines1.len() as u64);
    let headerless = hash_headerless(&ines1, &NES).unwrap();
    assert_eq!(Some(&headerless), hash_headerless(&nes2, &NES).as_ref());
    assert_eq!(headerless, hash_image(&body));
    // Trainer flag: 512 more bytes are skipped.
    let mut trained = ines1.clone();
    trained[6] |= 0x04;
    assert_eq!(
        hash_headerless(&trained, &NES),
        Some(hash_image(&body[512..]))
    );
}

#[test]
fn smd_copy_hashes_as_its_raw_image() {
    let mut raw: Vec<u8> = (0..0x20000u32).map(|i| (i * 11 % 251) as u8).collect();
    raw[0x100..0x104].copy_from_slice(b"SEGA");
    let smd = raw_to_smd(&raw);
    assert_eq!(
        hash_rom_bytes(&smd, &GENESIS),
        hash_rom_bytes(&raw, &GENESIS)
    );
    assert_eq!(hash_rom_bytes(&smd, &GENESIS).size, raw.len() as u64);
}

const N64_ORDERS: [N64ByteOrder; 3] = [
    N64ByteOrder::BigEndian,
    N64ByteOrder::ByteSwapped,
    N64ByteOrder::LittleEndian,
];

fn z64_rom() -> Vec<u8> {
    let mut rom: Vec<u8> = (0..0x4000u32).map(|i| (i * 23 % 251) as u8).collect();
    rom[..4].copy_from_slice(&[0x80, 0x37, 0x12, 0x40]);
    rom
}

#[test]
fn n64_orders_hash_as_z64() {
    let z64 = z64_rom();
    let expected = hash_image(&z64);
    for order in N64_ORDERS {
        let dump = n64_to_order(&z64, order);
        assert_eq!(hash_rom_bytes(&dump, &NINTENDO_64), expected, "{order:?}");
    }
}

#[test]
fn owned_file_hash_equals_borrowed_hash() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-hasher-")
        .tempdir()
        .unwrap();
    let z64 = z64_rom();
    for order in N64_ORDERS {
        let dump = n64_to_order(&z64, order);
        let path = temp.path().join(format!("{order:?}.n64"));
        std::fs::write(&path, &dump).unwrap();
        let (hashes, bytes) = hash_rom_file(&path, &NINTENDO_64).unwrap();
        assert_eq!(hashes, hash_rom_bytes(&dump, &NINTENDO_64), "{order:?}");
        assert_eq!(bytes, z64, "{order:?}: buffer comes back normalised");
    }
    let mut headered = vec![0xAA; 512];
    headered.extend_from_slice(&z64);
    let path = temp.path().join("copier.sfc");
    std::fs::write(&path, &headered).unwrap();
    let (hashes, bytes) = hash_rom_file(&path, &SNES).unwrap();
    assert_eq!(hashes, hash_rom_bytes(&headered, &SNES));
    assert_eq!(bytes, headered);
}

#[test]
fn whole_file_cartridges_over_the_threshold_are_streamed() {
    let temp = tempfile::Builder::new()
        .prefix("nostyra-hasher-")
        .tempdir()
        .unwrap();
    let path = temp.path().join("big.nds");
    let data: Vec<u8> = (0..3_000_000u32).map(|i| (i * 7 % 251) as u8).collect();
    std::fs::write(&path, &data).unwrap();
    let nds = system_catalog::NDS;
    let (full, bytes) = hash_rom_file_with(&path, &nds, u64::MAX).unwrap();
    assert_eq!(bytes, data, "under the threshold: read whole");
    let (streamed, bytes) = hash_rom_file_with(&path, &nds, 1024 * 1024).unwrap();
    assert_eq!(streamed, full, "same hashes and size");
    assert!(bytes.is_empty(), "streamed: no bytes");
    let small = SystemSpec {
        max_bytes: 2_000_000,
        ..nds
    };
    let refused = hash_rom_file_with(&path, &small, 1024 * 1024).unwrap_err();
    assert_eq!(refused.kind(), std::io::ErrorKind::InvalidData);
    // Header rules that rewrite bytes never stream.
    let (_, bytes) = hash_rom_file_with(&path, &SNES, 1024).unwrap();
    assert_eq!(bytes.len(), data.len());
}
