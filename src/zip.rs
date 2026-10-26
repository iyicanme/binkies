//! Minimal uncompressed (stored) ZIP writer.

/// Builds a ZIP archive with every file stored uncompressed.
pub fn store(files: &[(&str, &[u8])]) -> Vec<u8> {
    // 1980-01-01 00:00, the earliest DOS timestamp.
    const DOS_TIME: u16 = 0;
    const DOS_DATE: u16 = (1 << 5) | 1;

    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let (name, size) = (name.as_bytes(), data.len() as u32);

        put(&mut out, &[0x04034b50]);
        put16(&mut out, &[20, 0, 0, DOS_TIME, DOS_DATE]); // version, flags, method, time, date
        put(&mut out, &[crc, size, size]);
        put16(&mut out, &[name.len() as u16, 0]);
        out.extend_from_slice(name);
        out.extend_from_slice(data);

        put(&mut central, &[0x02014b50]);
        put16(&mut central, &[20, 20, 0, 0, DOS_TIME, DOS_DATE]); // made by, needed, flags, method, time, date
        put(&mut central, &[crc, size, size]);
        put16(&mut central, &[name.len() as u16, 0, 0, 0, 0]); // name, extra, comment, disk, internal attrs
        put(&mut central, &[0, offset]); // external attrs, local header offset
        central.extend_from_slice(name);
    }
    let (cd_offset, cd_size) = (out.len() as u32, central.len() as u32);
    out.extend_from_slice(&central);
    put(&mut out, &[0x06054b50]);
    put16(&mut out, &[0, 0, files.len() as u16, files.len() as u16]);
    put(&mut out, &[cd_size, cd_offset]);
    put16(&mut out, &[0]);
    out
}

fn put(out: &mut Vec<u8>, values: &[u32]) {
    values.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
}

fn put16(out: &mut Vec<u8>, values: &[u16]) {
    values.iter().for_each(|v| out.extend_from_slice(&v.to_le_bytes()));
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & (crc & 1).wrapping_neg());
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf43926);
    }

    #[test]
    fn archive_layout() {
        let zip = store(&[("a.txt", b"hello"), ("b.txt", b"")]);
        assert_eq!(&zip[..4], b"PK\x03\x04");
        assert_eq!(&zip[zip.len() - 22..zip.len() - 18], b"PK\x05\x06");
        assert_eq!(u16::from_le_bytes([zip[zip.len() - 12], zip[zip.len() - 11]]), 2);
    }
}
