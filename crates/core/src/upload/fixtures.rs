//! Tiny synthetic file headers; no real photos are ever used as fixtures.

const TAG_MAKE: u16 = 0x010F;
const TAG_DNG_VERSION: u16 = 0xC612;

pub fn jpeg(width: u16, height: u16) -> Vec<u8> {
    let mut bytes = vec![0xFF, 0xD8];
    // APP0 segment the parser must skip over.
    bytes.extend([0xFF, 0xE0, 0x00, 0x04, 0x00, 0x00]);
    // SOF0: length 11, 8-bit precision, one component.
    bytes.extend([0xFF, 0xC0, 0x00, 0x0B, 0x08]);
    bytes.extend(height.to_be_bytes());
    bytes.extend(width.to_be_bytes());
    bytes.extend([0x01, 0x01, 0x11, 0x00]);
    bytes.extend([0xFF, 0xD9]);
    bytes
}

pub fn cr2() -> Vec<u8> {
    let mut bytes = b"II*\0\x10\0\0\0CR\x02\0".to_vec();
    bytes.extend([0; 8]);
    bytes
}

pub fn cr3() -> Vec<u8> {
    let mut bytes = vec![0, 0, 0, 0x18];
    bytes.extend(b"ftypcrx \0\0\0\x01crx isom");
    bytes
}

pub fn raf() -> Vec<u8> {
    let mut bytes = b"FUJIFILMCCD-RAW ".to_vec();
    bytes.extend(b"0201FF129503");
    bytes
}

/// TIFF container with a Make tag and/or a DNGVersion tag in IFD0.
pub fn tiff(big_endian: bool, make: Option<&str>, dng: bool) -> Vec<u8> {
    let u16_bytes = |value: u16| {
        if big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        }
    };
    let u32_bytes = |value: u32| {
        if big_endian {
            value.to_be_bytes()
        } else {
            value.to_le_bytes()
        }
    };

    let mut entries: Vec<(u16, Vec<u8>)> = Vec::new();
    let entry_count = usize::from(make.is_some()) + usize::from(dng);
    let data_start = 8 + 2 + entry_count * 12 + 4;

    let mut data = Vec::new();
    if let Some(make) = make {
        let mut entry = Vec::new();
        entry.extend(u16_bytes(2));
        entry.extend(u32_bytes(make.len() as u32 + 1));
        entry.extend(u32_bytes((data_start + data.len()) as u32));
        data.extend(make.as_bytes());
        data.push(0);
        entries.push((TAG_MAKE, entry));
    }
    if dng {
        let mut entry = Vec::new();
        entry.extend(u16_bytes(1));
        entry.extend(u32_bytes(4));
        entry.extend([1, 4, 0, 0]);
        entries.push((TAG_DNG_VERSION, entry));
    }

    let mut bytes = Vec::new();
    bytes.extend(if big_endian { b"MM\0*" } else { b"II*\0" });
    bytes.extend(u32_bytes(8));
    bytes.extend(u16_bytes(entry_count as u16));
    for (tag, entry) in entries {
        bytes.extend(u16_bytes(tag));
        bytes.extend(entry);
    }
    bytes.extend(u32_bytes(0));
    bytes.extend(data);
    bytes
}

pub fn nef() -> Vec<u8> {
    tiff(true, Some("NIKON CORPORATION"), false)
}

pub fn arw() -> Vec<u8> {
    tiff(false, Some("SONY"), false)
}

pub fn dng() -> Vec<u8> {
    tiff(false, Some("Leica Camera AG"), true)
}
