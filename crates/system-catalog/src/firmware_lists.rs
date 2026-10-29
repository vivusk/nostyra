//! BIOS file lists of the cores that use one (`CoreSpec.firmware`).

use crate::core_specs::FirmwareFile;
use crate::firmware_status::{ANY_REGION, BIOS_BYTES};

/// PlayStation BIOS files PCSX ReARMed looks for, in its lookup order (libretro
/// core info + docs; md5 identifies a picked file and gives its canonical name).
pub(crate) const PCSX_REARMED_BIOS: &[FirmwareFile] = &[
    bios(
        "scph5500.bin",
        "8dd7d5296a650fac7319bce665a6a53c",
        "JP",
        "SCPH-5500",
    ),
    bios(
        "scph5501.bin",
        "490f666e1afb15b7362b406ed1cea246",
        "US",
        "SCPH-5501",
    ),
    bios(
        "scph5502.bin",
        "32736f17079d0b2b7024407c39bd3050",
        "EU",
        "SCPH-5502",
    ),
    bios(
        "psxonpsp660.bin",
        "c53ca5908936d412331790f4426c6c33",
        "ANY",
        "PSP 6.60",
    ),
    bios(
        "scph101.bin",
        "6e3735ff4c7dc899ee98981385f6f3d0",
        "US",
        "SCPH-101",
    ),
    bios(
        "scph7001.bin",
        "1e68c231d0896b7eadcad1d7d8e76129",
        "US",
        "SCPH-7001",
    ),
    bios(
        "scph1001.bin",
        "924e392ed05558ffdb115408c263dccf",
        "US",
        "SCPH-1001",
    ),
];

const fn bios(
    name: &'static str,
    md5: &'static str,
    region: &'static str,
    model: &'static str,
) -> FirmwareFile {
    FirmwareFile {
        name,
        md5,
        region,
        model,
        size: BIOS_BYTES,
        subdir: "",
    }
}

/// Dreamcast BIOS Flycast boots when present in `<system>/dc/` (2 MiB; md5 from the
/// libretro core info). Without it Flycast's HLE BIOS (reios) runs.
pub(crate) const FLYCAST_BIOS: &[FirmwareFile] = &[FirmwareFile {
    name: "dc_boot.bin",
    md5: "e10c53c2f8b90bab96ead2d368858623",
    region: ANY_REGION,
    model: "Dreamcast",
    size: 2 * 1024 * 1024,
    subdir: "dc",
}];
