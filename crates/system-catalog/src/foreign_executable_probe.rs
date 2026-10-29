//! Quiet pre-check for GameCube / Wii `.elf` candidates: an ELF built for another
//! CPU (PSP / Dreamcast / PS2 homebrew, a PC binary) is no GameCube / Wii file at
//! all, so the scanner skips it silently ([`ClassifyError::NotRom`](crate::ClassifyError))
//! instead of listing a problem. Decided from the ELF identification bytes in the
//! file head; a PowerPC ELF32 big-endian file (even a broken one) and anything
//! without the ELF magic go on to the disc probe, which reports real problems.

use crate::disc_probe::DiscSignature;

/// Reason shown by the runner for a non-PowerPC `.elf` (the scanner shows nothing).
pub(crate) const FOREIGN_ELF: &str = "not a PowerPC executable";

const ELF_EXT: &str = "elf";
const ELF_MAGIC: &[u8] = b"\x7FELF";
/// `e_ident[EI_CLASS]` = ELFCLASS32, `e_ident[EI_DATA]` = ELFDATA2MSB.
const CLASS_32: u8 = 1;
const BIG_ENDIAN: u8 = 2;
/// `e_machine` (BE u16 @0x12) = EM_PPC.
const MACHINE_PPC: u16 = 20;
/// Bytes up to and including `e_machine`.
pub(crate) const ELF_HEAD_BYTES: usize = 0x14;

/// True when `signature` checks a file with extension `ext` against its head bytes
/// first (GameCube / Wii `.elf`).
pub(crate) fn checks_elf_head(signature: DiscSignature, ext: &str) -> bool {
    matches!(signature, DiscSignature::GameCube | DiscSignature::Wii)
        && ext.eq_ignore_ascii_case(ELF_EXT)
}

/// `Some(reason)` when `head` is a well-formed ELF identification for a CPU other
/// than 32-bit big-endian PowerPC.
pub(crate) fn foreign_elf(head: &[u8]) -> Option<&'static str> {
    if !head.starts_with(ELF_MAGIC) || head.len() < ELF_HEAD_BYTES {
        return None;
    }
    let machine = u16::from_be_bytes([head[0x12], head[0x13]]);
    let ppc = head[4] == CLASS_32 && head[5] == BIG_ENDIAN && machine == MACHINE_PPC;
    (!ppc).then_some(FOREIGN_ELF)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elf_head(class: u8, data: u8, machine: [u8; 2]) -> Vec<u8> {
        let mut head = b"\x7FELF".to_vec();
        head.extend([class, data, 1]);
        head.resize(0x12, 0);
        head.extend(machine);
        head
    }

    #[test]
    fn only_32_bit_big_endian_powerpc_elves_go_to_the_disc_probe() {
        assert_eq!(foreign_elf(&elf_head(1, 2, [0, 20])), None, "PowerPC");
        // MIPS LE (PSP), SH-4 LE (Dreamcast), x86-64, PowerPC64, PowerPC LE.
        for head in [
            elf_head(1, 1, [8, 0]),
            elf_head(1, 1, [42, 0]),
            elf_head(2, 1, [62, 0]),
            elf_head(2, 2, [0, 21]),
            elf_head(1, 1, [20, 0]),
        ] {
            assert_eq!(foreign_elf(&head), Some(FOREIGN_ELF), "{head:?}");
        }
        // No magic or a cut header: the disc probe decides (and reports).
        assert_eq!(foreign_elf(b"garbage"), None);
        assert_eq!(foreign_elf(&elf_head(1, 1, [8, 0])[..0x10]), None);
    }

    #[test]
    fn only_gamecube_and_wii_elves_are_pre_checked() {
        assert!(checks_elf_head(DiscSignature::GameCube, "ELF"));
        assert!(checks_elf_head(DiscSignature::Wii, "elf"));
        assert!(!checks_elf_head(DiscSignature::Wii, "dol"));
        assert!(!checks_elf_head(DiscSignature::Psp, "elf"));
    }
}
