//! Lists what is inside a pack file without unpacking it.
//!
//! Zips: reads only the end record and the central directory (one read near the end
//! of the file, one of the directory itself), like Python's `zipfile.infolist()`. The
//! `zip` crate seeks to every entry's local header, which is 50,000 seeks on a disk.
//! Unity packages: a gzipped tar of `<guid>/asset` + `<guid>/pathname`, streamed once.

use std::{
    collections::HashMap,
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

/// One file inside a pack.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub name: String,
    /// Unpacked size in bytes.
    pub size: u64,
    /// Where the entry's local header starts (zips only), for reading it later.
    pub offset: u64,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes(b[i..i + 4].try_into().unwrap())
}
fn u64_at(b: &[u8], i: usize) -> u64 {
    u64::from_le_bytes(b[i..i + 8].try_into().unwrap())
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

/// IBM PC code page 437, bytes 0x80..=0xFF: the zip default when the UTF-8 flag is off.
const CP437_HIGH: &str = "ÇüéâäàåçêëèïîìÄÅÉæÆôöòûùÿÖÜ¢£¥₧ƒáíóúñÑªº¿⌐¬½¼¡«»░▒▓│┤╡╢╖╕╣║╗╝╜╛┐└┴┬├─┼╞╟╚╔╩╦╠═╬╧╨╤╥╙╘╒╓╫╪┘┌█▄▌▐▀αßΓπΣσµτΦΘΩδ∞φε∩≡±≥≤⌠⌡÷≈°∙·√ⁿ²■\u{a0}";

fn cp437(bytes: &[u8]) -> String {
    let high: Vec<char> = CP437_HIGH.chars().collect();
    bytes.iter().map(|&b| if b < 0x80 { b as char } else { high[(b - 0x80) as usize] }).collect()
}

/// Every file entry (folders left out) of the zip at `path`.
pub fn zip_entries(path: &Path) -> io::Result<Vec<Entry>> {
    let mut f = File::open(path)?;
    let len = f.metadata()?.len();
    if len < 22 {
        return Err(bad("too small for a zip"));
    }
    // End of central directory: 22 bytes plus a comment of up to 64 KB, at the very end.
    let tail_len = len.min(22 + 65_535 + 20);
    let mut tail = vec![0; tail_len as usize];
    f.seek(SeekFrom::Start(len - tail_len))?;
    f.read_exact(&mut tail)?;
    let eocd = (0..=tail.len() - 22)
        .rev()
        .find(|&i| u32_at(&tail, i) == 0x0605_4b50)
        .ok_or_else(|| bad("no end of central directory: not a zip"))?;
    let mut total = u16_at(&tail, eocd + 10) as u64;
    let mut cd_size = u32_at(&tail, eocd + 12) as u64;
    let mut cd_offset = u32_at(&tail, eocd + 16) as u64;
    let eocd_pos = len - tail_len + eocd as u64;
    // Zip64: the real numbers are in a second end record, found through a locator.
    if (total == 0xFFFF || cd_size == 0xFFFF_FFFF || cd_offset == 0xFFFF_FFFF)
        && eocd >= 20
        && u32_at(&tail, eocd - 20) == 0x0706_4b50
    {
        let at = u64_at(&tail, eocd - 20 + 8);
        let mut rec = [0u8; 56];
        f.seek(SeekFrom::Start(at))?;
        f.read_exact(&mut rec)?;
        if u32_at(&rec, 0) != 0x0606_4b50 {
            return Err(bad("broken zip64 end record"));
        }
        total = u64_at(&rec, 32);
        cd_size = u64_at(&rec, 40);
        cd_offset = u64_at(&rec, 48);
    }
    if cd_size > 512 * 1024 * 1024 || cd_size > len {
        return Err(bad("central directory too large"));
    }
    // Self-extracting zips have data in front: offsets are then off by that much.
    let shift = eocd_pos.saturating_sub(cd_size).saturating_sub(cd_offset);
    let mut cd = vec![0; cd_size as usize];
    let read_cd = |f: &mut File, cd: &mut Vec<u8>, at: u64| -> io::Result<bool> {
        f.seek(SeekFrom::Start(at))?;
        f.read_exact(cd)?;
        Ok(cd.len() < 4 || u32_at(cd, 0) == 0x0201_4b50)
    };
    let shift = if read_cd(&mut f, &mut cd, cd_offset)? {
        0
    } else if shift > 0 && cd_offset + shift + cd_size <= len && read_cd(&mut f, &mut cd, cd_offset + shift)? {
        shift
    } else {
        return Err(bad("central directory not where the end record says"));
    };
    let mut out = Vec::with_capacity(total.min(1_000_000) as usize);
    let mut i = 0;
    while i + 46 <= cd.len() && u32_at(&cd, i) == 0x0201_4b50 {
        let flags = u16_at(&cd, i + 8);
        let mut size = u32_at(&cd, i + 24) as u64;
        let csize = u32_at(&cd, i + 20);
        let name_len = u16_at(&cd, i + 28) as usize;
        let extra_len = u16_at(&cd, i + 30) as usize;
        let comment_len = u16_at(&cd, i + 32) as usize;
        let mut offset = u32_at(&cd, i + 42) as u64;
        let end = i + 46 + name_len + extra_len + comment_len;
        if end > cd.len() {
            return Err(bad("central directory entry runs past its end"));
        }
        let raw = &cd[i + 46..i + 46 + name_len];
        let mut name = if flags & 0x0800 != 0 { String::from_utf8_lossy(raw).into_owned() } else { cp437(raw) };
        // Extra fields: zip64 sizes (0x0001) and the Info-ZIP Unicode path (0x7075).
        let extra = &cd[i + 46 + name_len..i + 46 + name_len + extra_len];
        let mut j = 0;
        while j + 4 <= extra.len() {
            let id = u16_at(extra, j);
            let n = u16_at(extra, j + 2) as usize;
            let body = &extra[(j + 4).min(extra.len())..(j + 4 + n).min(extra.len())];
            if id == 0x0001 {
                let mut k = 0;
                if size == 0xFFFF_FFFF && body.len() >= k + 8 {
                    size = u64_at(body, k);
                    k += 8;
                }
                if csize == 0xFFFF_FFFF && body.len() >= k + 8 {
                    k += 8;
                }
                if offset == 0xFFFF_FFFF && body.len() >= k + 8 {
                    offset = u64_at(body, k);
                }
            } else if id == 0x7075 && body.len() > 5 && flags & 0x0800 == 0 {
                if let Ok(s) = std::str::from_utf8(&body[5..]) {
                    name = s.to_string();
                }
            }
            j += 4 + n;
        }
        i = end;
        // Folders end in "/" (Windows tools sometimes write "\").
        let name = name.replace('\\', "/");
        if name.ends_with('/') {
            continue;
        }
        out.push(Entry { name, size, offset: offset + shift });
    }
    Ok(out)
}

/// Every asset in a Unity package, by the path it gets in the Unity project.
pub fn unitypackage_entries(path: &Path) -> io::Result<Vec<Entry>> {
    let gz = flate2::read::GzDecoder::new(io::BufReader::with_capacity(1 << 20, File::open(path)?));
    let mut tar = tar::Archive::new(gz);
    let mut names: HashMap<String, String> = HashMap::new();
    let mut sizes: HashMap<String, u64> = HashMap::new();
    for e in tar.entries()? {
        let mut e = e?;
        let p = e.path()?.to_string_lossy().replace('\\', "/");
        let p = p.trim_start_matches("./");
        let Some((guid, file)) = p.split_once('/') else { continue };
        match file {
            "pathname" => {
                let mut s = String::new();
                e.by_ref().take(4096).read_to_string(&mut s)?;
                // The first line is the path; some exporters add a second line.
                names.insert(guid.to_string(), s.lines().next().unwrap_or("").trim().to_string());
            }
            "asset" => {
                sizes.insert(guid.to_string(), e.header().size()?);
            }
            _ => {}
        }
    }
    let mut out: Vec<Entry> = sizes
        .into_iter()
        .filter_map(|(guid, size)| names.remove(&guid).filter(|n| !n.is_empty()).map(|name| Entry { name, size, offset: 0 }))
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A stored (uncompressed) zip with a folder and two files, one with a non-UTF-8 name.
    fn tiny_zip() -> Vec<u8> {
        let files: [(&[u8], &[u8], u16); 3] =
            [(b"Tracks/", b"", 0x0800), (b"Tracks/Action 1.mp3", b"abcde", 0x0800), (b"caf\x82.wav", b"xy", 0)];
        let (mut out, mut cd) = (vec![], vec![]);
        for (name, data, flags) in files {
            let off = out.len() as u32;
            out.extend(0x0403_4b50u32.to_le_bytes());
            out.extend([20, 0]);
            out.extend(flags.to_le_bytes());
            out.extend([0u8; 10]); // method, time, date, crc
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((data.len() as u32).to_le_bytes());
            out.extend((name.len() as u16).to_le_bytes());
            out.extend([0, 0]);
            out.extend(name);
            out.extend(data);
            cd.extend(0x0201_4b50u32.to_le_bytes());
            cd.extend([20, 0, 20, 0]);
            cd.extend(flags.to_le_bytes());
            cd.extend([0u8; 10]);
            cd.extend((data.len() as u32).to_le_bytes());
            cd.extend((data.len() as u32).to_le_bytes());
            cd.extend((name.len() as u16).to_le_bytes());
            cd.extend([0u8; 8]); // extra, comment, disk, internal attributes
            cd.extend([0u8; 4]); // external attributes
            cd.extend(off.to_le_bytes());
            cd.extend(name);
        }
        let cd_off = out.len() as u32;
        out.extend(&cd);
        out.extend(0x0605_4b50u32.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend(3u16.to_le_bytes());
        out.extend(3u16.to_le_bytes());
        out.extend((cd.len() as u32).to_le_bytes());
        out.extend(cd_off.to_le_bytes());
        out.extend([0, 0]);
        out
    }

    #[test]
    fn reads_the_central_directory() {
        let dir = std::env::temp_dir().join(format!("kk-zip-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("pack.zip");
        std::fs::write(&p, tiny_zip()).unwrap();
        let e = zip_entries(&p).unwrap();
        assert_eq!(e.len(), 2, "the folder entry is left out: {e:?}");
        assert_eq!(e[0].name, "Tracks/Action 1.mp3");
        assert_eq!(e[0].size, 5);
        assert_eq!(e[1].name, "café.wav", "code page 437 without the UTF-8 flag");
        // The offset points at the entry's local header.
        let bytes = std::fs::read(&p).unwrap();
        assert_eq!(u32_at(&bytes, e[1].offset as usize), 0x0403_4b50);
        std::fs::write(&p, b"not a zip at all, just text").unwrap();
        assert!(zip_entries(&p).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reads_unity_package_paths() {
        let dir = std::env::temp_dir().join(format!("kk-unity-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("pack.unitypackage");
        let gz = flate2::write::GzEncoder::new(File::create(&p).unwrap(), flate2::Compression::fast());
        let mut t = tar::Builder::new(gz);
        let mut add = |path: &str, data: &[u8]| {
            let mut h = tar::Header::new_gnu();
            h.set_size(data.len() as u64);
            h.set_mode(0o644);
            h.set_cksum();
            t.append_data(&mut h, path, data).unwrap();
        };
        add("abc/pathname", b"Assets/Goblin/Run.fbx\n00");
        add("abc/asset", b"fbxdata");
        add("abc/asset.meta", b"meta");
        add("def/pathname", b"Assets/Goblin"); // a folder: no asset file
        t.into_inner().unwrap().finish().unwrap().flush().unwrap();
        let e = unitypackage_entries(&p).unwrap();
        assert_eq!(e, vec![Entry { name: "Assets/Goblin/Run.fbx".into(), size: 7, offset: 0 }]);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
