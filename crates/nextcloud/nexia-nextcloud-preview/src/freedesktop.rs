//! FreeDesktop Thumbnail Managing Standard integration for Linux/desktop file managers.
//!
//! Registers Nextcloud-provided preview thumbnails into the standard system cache
//! (`$XDG_CACHE_HOME/thumbnails/{normal,large,x-large}/`) with required `Thumb::URI` and `Thumb::MTime`
//! PNG metadata chunks so desktop file managers (GNOME Files/Nautilus, Dolphin, Thunar)
//! display them immediately without attempting to thumbnail 0-byte or unhydrated virtual placeholders.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Computes the `file://` URI for a local path according to RFC 3986 without resolving symlinks.
pub fn path_to_file_uri(path: &Path) -> String {
    let path_str = path.to_string_lossy();
    let mut uri = String::from("file://");
    for b in path_str.bytes() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                uri.push(b as char);
            }
            _ => {
                uri.push_str(&format!("%{:02X}", b));
            }
        }
    }
    uri
}

/// Collects distinct `file://` URIs for `target_file`, including both raw and canonical paths.
pub fn file_uris_for_path(path: &Path) -> Vec<String> {
    let mut uris = Vec::new();
    let raw_uri = path_to_file_uri(path);
    uris.push(raw_uri.clone());

    if let Ok(canonical) = path.canonicalize() {
        let can_uri = path_to_file_uri(&canonical);
        if can_uri != raw_uri {
            uris.push(can_uri);
        }
    }
    uris
}

/// Standard IEEE 802.3 CRC32 calculation over a byte slice.
#[must_use]
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in data {
        crc ^= b as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Computes MD5 checksum as a 32-character lowercase hexadecimal string per RFC 1321.
#[must_use]
pub fn md5_hex(input: &[u8]) -> String {
    let mut a0 = 0x6745_2301u32;
    let mut b0 = 0xEFCD_AB89u32;
    let mut c0 = 0x98BA_DCFEu32;
    let mut d0 = 0x1032_5476u32;

    let s: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 5, 9, 14, 20, 5, 9, 14, 20, 5,
        9, 14, 20, 5, 9, 14, 20, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 6, 10,
        15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let k: [u32; 64] = [
        0xD76A_A478,
        0xE8C7_B756,
        0x2420_70DB,
        0xC1BD_CEEE,
        0xF57C_0FAF,
        0x4787_C62A,
        0xA830_4613,
        0xFD46_9501,
        0x6980_98D8,
        0x8B44_F7AF,
        0xFFFF_5BB1,
        0x895C_D7BE,
        0x6B90_1122,
        0xFD98_7193,
        0xA679_438E,
        0x49B4_0821,
        0xF61E_2562,
        0xC040_B340,
        0x265E_5A51,
        0xE9B6_C7AA,
        0xD62F_105D,
        0x0244_1453,
        0xD8A1_E681,
        0xE7D3_FBC8,
        0x21E1_CDE6,
        0xC337_07D6,
        0xF4D5_0D87,
        0x455A_14ED,
        0xA9E3_E905,
        0xFCEF_A3F8,
        0x676F_02D9,
        0x8D2A_4C8A,
        0xFFFA_3942,
        0x8771_F681,
        0x6D9D_6122,
        0xFDE5_380C,
        0xA4BE_EA44,
        0x4BDE_CFA9,
        0xF6BB_4B60,
        0xBEBF_BC70,
        0x289B_7EC6,
        0xEAA1_27FA,
        0xD4EF_3085,
        0x0488_1D05,
        0xD9D4_D039,
        0xE6DB_99E5,
        0x1FA2_7CF8,
        0xC4AC_5665,
        0xF429_2244,
        0x432A_FF97,
        0xAB94_23A7,
        0xFC93_A039,
        0x655B_59C3,
        0x8F0C_CC92,
        0xFFEF_F47D,
        0x8584_5DD1,
        0x6FA8_7E4F,
        0xFE2C_E6E0,
        0xA301_4314,
        0x4E08_11A1,
        0xF753_7E82,
        0xBD3A_F235,
        0x2AD7_D2BB,
        0xEB86_D391,
    ];

    let mut padded = input.to_vec();
    let orig_len_bits = (input.len() as u64) * 8;
    padded.push(0x80);
    while (padded.len() % 64) != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&orig_len_bits.to_le_bytes());

    for chunk in padded.as_chunks::<64>().0 {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }

        let mut a = a0;
        let mut b = b0;
        let mut c = c0;
        let mut d = d0;

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | ((!b) & d), i),
                16..=31 => ((d & b) | ((!d) & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | (!d)), (7 * i) % 16),
            };

            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                (a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g])).rotate_left(s[i]),
            );
            a = temp;
        }

        a0 = a0.wrapping_add(a);
        b0 = b0.wrapping_add(b);
        c0 = c0.wrapping_add(c);
        d0 = d0.wrapping_add(d);
    }

    let mut result = [0u8; 16];
    result[0..4].copy_from_slice(&a0.to_le_bytes());
    result[4..8].copy_from_slice(&b0.to_le_bytes());
    result[8..12].copy_from_slice(&c0.to_le_bytes());
    result[12..16].copy_from_slice(&d0.to_le_bytes());

    let mut hex = String::with_capacity(32);
    for b in result {
        hex.push_str(&format!("{b:02x}"));
    }
    hex
}

/// Builds a PNG `tEXt` chunk containing `keyword` and `text`.
pub fn make_png_text_chunk(keyword: &str, text: &str) -> Vec<u8> {
    let mut data = Vec::with_capacity(keyword.len() + 1 + text.len());
    data.extend_from_slice(keyword.as_bytes());
    data.push(0);
    data.extend_from_slice(text.as_bytes());

    let mut chunk = Vec::with_capacity(4 + 4 + data.len() + 4);
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(b"tEXt");
    chunk.extend_from_slice(&data);

    let mut crc_payload = Vec::with_capacity(4 + data.len());
    crc_payload.extend_from_slice(b"tEXt");
    crc_payload.extend_from_slice(&data);
    let crc = crc32(&crc_payload);
    chunk.extend_from_slice(&crc.to_be_bytes());

    chunk
}

/// Converts raw image bytes (such as JPEG returned by Nextcloud previews) to standard PNG format if needed,
/// resizing to a maximum thumbnail boundary of 512x512 if larger.
#[must_use]
pub fn ensure_png_bytes(bytes: &[u8]) -> Vec<u8> {
    const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if let Ok(img) = image::load_from_memory(bytes) {
        if img.width() > 512 || img.height() > 512 {
            let resized = img.thumbnail(512, 512);
            let mut out = std::io::Cursor::new(Vec::with_capacity(64 * 1024));
            if resized.write_to(&mut out, image::ImageFormat::Png).is_ok() {
                return out.into_inner();
            }
        } else if !bytes.starts_with(PNG_SIG) {
            let mut out = std::io::Cursor::new(Vec::with_capacity(bytes.len() + 1024));
            if img.write_to(&mut out, image::ImageFormat::Png).is_ok() {
                return out.into_inner();
            }
        }
    }

    bytes.to_vec()
}

/// Injects FreeDesktop `Thumb::URI`, `Thumb::MTime` and `Software` chunks directly into
/// raw PNG bytes without re-encoding.
///
/// `software` fills the optional `Software` chunk, which the Thumbnail Managing Standard
/// defines as the name of the program that generated the thumbnail. Only the caller
/// knows that name, so it is never defaulted here; `None` omits the chunk, which is
/// correct for sentinel markers that are not generated thumbnails.
#[must_use]
pub fn embed_freedesktop_metadata_raw(
    png_bytes: &[u8],
    uri: &str,
    mtime_sec: u64,
    software: Option<&str>,
) -> Vec<u8> {
    const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";
    if !png_bytes.starts_with(PNG_SIG) || png_bytes.len() < 33 {
        return png_bytes.to_vec();
    }

    let chunk_uri = make_png_text_chunk("Thumb::URI", uri);
    let chunk_mtime = make_png_text_chunk("Thumb::MTime", &mtime_sec.to_string());
    let chunk_software = software.map(|name| make_png_text_chunk("Software", name));

    let total_len = png_bytes.len()
        + chunk_uri.len()
        + chunk_mtime.len()
        + chunk_software.as_ref().map_or(0, Vec::len);
    let mut result = Vec::with_capacity(total_len);

    // PNG signature (8 bytes) + IHDR chunk (25 bytes: 4 len + 4 type + 13 data + 4 crc)
    result.extend_from_slice(&png_bytes[..33]);

    // Insert metadata chunks immediately after IHDR
    result.extend_from_slice(&chunk_uri);
    result.extend_from_slice(&chunk_mtime);
    if let Some(chunk) = &chunk_software {
        result.extend_from_slice(chunk);
    }

    // Append remainder of original PNG
    result.extend_from_slice(&png_bytes[33..]);
    result
}

/// Resizes `img` to fit within `(max_dim, max_dim)` and encodes to PNG tagged with FreeDesktop metadata.
#[must_use]
pub fn create_freedesktop_png(
    img: &image::DynamicImage,
    max_dim: u32,
    uri: &str,
    mtime_sec: u64,
    software: &str,
) -> Option<Vec<u8>> {
    let resized = if img.width() > max_dim || img.height() > max_dim {
        img.thumbnail(max_dim, max_dim)
    } else {
        img.clone()
    };
    let mut out = std::io::Cursor::new(Vec::with_capacity(32 * 1024));
    resized.write_to(&mut out, image::ImageFormat::Png).ok()?;
    let png_bytes = out.into_inner();
    Some(embed_freedesktop_metadata_raw(
        &png_bytes,
        uri,
        mtime_sec,
        Some(software),
    ))
}

/// Injects FreeDesktop `Thumb::URI`, `Thumb::MTime` and `Software` chunks into PNG image bytes.
///
/// `software` names the program generating the thumbnail; see
/// [`embed_freedesktop_metadata_raw`].
pub fn embed_freedesktop_metadata(
    raw_bytes: &[u8],
    uri: &str,
    mtime_sec: u64,
    software: &str,
) -> Vec<u8> {
    let png_bytes = ensure_png_bytes(raw_bytes);
    embed_freedesktop_metadata_raw(&png_bytes, uri, mtime_sec, Some(software))
}

/// Locates standard FreeDesktop thumbnail directories (`$XDG_CACHE_HOME/thumbnails/{normal,large}`).
pub fn freedesktop_thumbnail_dirs() -> (PathBuf, PathBuf) {
    let base_cache = if let Ok(xdg) = std::env::var("XDG_CACHE_HOME") {
        PathBuf::from(xdg)
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".cache")
    } else if let Ok(userprofile) = std::env::var("USERPROFILE") {
        PathBuf::from(userprofile).join(".cache")
    } else {
        std::env::temp_dir()
    };

    let thumbs = base_cache.join("thumbnails");
    (thumbs.join("normal"), thumbs.join("large"))
}

/// Installs a Nextcloud-provided preview into the FreeDesktop thumbnail cache for `target_file`.
///
/// `software` names the program generating the thumbnail and is written to the PNG
/// `Software` chunk.
pub fn install_freedesktop_thumbnail(target_file: &Path, png_bytes: &[u8], software: &str) {
    let mtime_sec = target_file
        .metadata()
        .and_then(|m| m.modified())
        .unwrap_or_else(|_| SystemTime::now())
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    install_freedesktop_thumbnail_with_mtime(target_file, png_bytes, mtime_sec, software);
}

/// Returns whether a thumbnail for `target_file` is already installed at every standard size.
///
/// The FreeDesktop standard treats the thumbnail cache as authoritative, so a caller that
/// finds every expected size present has nothing left to produce. Checking that before
/// [`install_freedesktop_thumbnail_with_mtime`] matters because installing decodes and
/// re-encodes the image even when every destination already exists: a directory listing
/// that reinstalls a few hundred thumbnails burns seconds of CPU and rewrites the very
/// files a file manager is reading back, which is visible as stutter while a folder opens.
pub fn freedesktop_thumbnail_is_current(target_file: &Path) -> bool {
    let (normal_dir, large_dir) = freedesktop_thumbnail_dirs();
    let Some(root) = normal_dir.parent().map(Path::to_path_buf) else {
        return false;
    };
    let dirs = [
        normal_dir,
        large_dir,
        root.join("x-large"),
        root.join("xx-large"),
    ];

    thumbnails_current_in(&dirs, target_file)
}

/// Returns whether every URI form of `target_file` has a thumbnail of every size in `dirs`.
///
/// Split out from [`freedesktop_thumbnail_is_current`] so the bookkeeping can be exercised against
/// a directory of the caller's choosing: the public entry point resolves the cache from the process
/// environment, which this crate is not allowed to write to.
fn thumbnails_current_in(dirs: &[PathBuf], target_file: &Path) -> bool {
    let uris = file_uris_for_path(target_file);
    if uris.is_empty() {
        return false;
    }

    uris.iter().all(|uri| {
        let filename = format!("{}.png", md5_hex(uri.as_bytes()));
        dirs.iter().all(|dir| dir.join(&filename).is_file())
    })
}

/// Installs a Nextcloud-provided preview into the FreeDesktop thumbnail cache for `target_file` with explicit modification time.
pub fn install_freedesktop_thumbnail_with_mtime(
    target_file: &Path,
    png_bytes: &[u8],
    mtime_sec: u64,
    software: &str,
) {
    let (normal_dir, large_dir) = freedesktop_thumbnail_dirs();
    let _ = fs::create_dir_all(&normal_dir);
    let _ = fs::create_dir_all(&large_dir);

    let xlarge_dir = normal_dir
        .parent()
        .map(|p| p.join("x-large"))
        .unwrap_or_else(|| large_dir.clone());
    let _ = fs::create_dir_all(&xlarge_dir);

    let xxlarge_dir = normal_dir
        .parent()
        .map(|p| p.join("xx-large"))
        .unwrap_or_else(|| large_dir.clone());
    let _ = fs::create_dir_all(&xxlarge_dir);

    if freedesktop_thumbnail_is_current(target_file) {
        return;
    }

    let parsed_img = image::load_from_memory(png_bytes).ok();

    let uris = file_uris_for_path(target_file);
    for uri in &uris {
        let hash = md5_hex(uri.as_bytes());
        let filename = format!("{hash}.png");

        let normal_dest = normal_dir.join(&filename);
        let large_dest = large_dir.join(&filename);
        let xlarge_dest = xlarge_dir.join(&filename);
        let xxlarge_dest = xxlarge_dir.join(&filename);

        if normal_dest.exists()
            && large_dest.exists()
            && xlarge_dest.exists()
            && xxlarge_dest.exists()
        {
            continue;
        }

        if let Some(ref img) = parsed_img {
            if let Some(normal_png) = create_freedesktop_png(img, 128, uri, mtime_sec, software) {
                let _ = fs::write(&normal_dest, normal_png);
            }
            if let Some(large_png) = create_freedesktop_png(img, 256, uri, mtime_sec, software) {
                let _ = fs::write(&large_dest, large_png);
            }
            if let Some(xlarge_png) = create_freedesktop_png(img, 512, uri, mtime_sec, software) {
                let _ = fs::write(&xlarge_dest, xlarge_png);
            }
            if let Some(xxlarge_png) = create_freedesktop_png(img, 1024, uri, mtime_sec, software) {
                let _ = fs::write(&xxlarge_dest, xxlarge_png);
            }
        } else {
            let tagged_bytes = embed_freedesktop_metadata(png_bytes, uri, mtime_sec, software);
            let _ = fs::write(&normal_dest, &tagged_bytes);
            let _ = fs::write(&large_dest, &tagged_bytes);
            let _ = fs::write(&xlarge_dest, &tagged_bytes);
            let _ = fs::write(&xxlarge_dest, &tagged_bytes);
        }

        if let Some(parent) = normal_dir.parent() {
            let fail_file = parent
                .join("fail")
                .join("gnome-thumbnail-factory")
                .join(&filename);
            if fail_file.exists() {
                let _ = fs::remove_file(fail_file);
            }
            let fail_generic = parent.join("fail").join(&filename);
            if fail_generic.exists() {
                let _ = fs::remove_file(fail_generic);
            }
        }
    }
}

/// Installs a FreeDesktop "fail" marker thumbnail for `target_file`.
///
/// When desktop file managers (like GNOME Files / Nautilus) check the thumbnail cache,
/// the presence of this fail marker in `$XDG_CACHE_HOME/thumbnails/fail/gnome-thumbnail-factory/`
/// informs them that thumbnail generation has failed or should not be retried locally.
/// This prevents Nautilus from launching thumbnailers that open and read unhydrated virtual files.
pub fn install_freedesktop_fail_marker(target_file: &Path, mtime_sec: u64) {
    let minimal_png = [
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG magic
        0x00, 0x00, 0x00, 0x0D, // IHDR length (13)
        0x49, 0x48, 0x44, 0x52, // IHDR type
        0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
        0x00, // 1x1 RGBA
        0x1F, 0x15, 0xC4, 0x89, // CRC
        0x00, 0x00, 0x00, 0x0A, // IDAT length (10)
        0x49, 0x44, 0x41, 0x54, // IDAT
        0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00,
        0x00, 0x00, 0x00, // IEND length (0)
        0x49, 0x45, 0x4E, 0x44, // IEND
        0xAE, 0x42, 0x60, 0x82, // CRC
    ];

    let (normal_dir, _) = freedesktop_thumbnail_dirs();
    let parent = normal_dir.parent();
    if let Some(parent_dir) = parent {
        let fail_factory_dir = parent_dir.join("fail").join("gnome-thumbnail-factory");
        let fail_generic_dir = parent_dir.join("fail");
        let _ = fs::create_dir_all(&fail_factory_dir);
        let _ = fs::create_dir_all(&fail_generic_dir);

        let uris = file_uris_for_path(target_file);
        for uri in uris {
            let tagged_bytes = embed_freedesktop_metadata_raw(&minimal_png, &uri, mtime_sec, None);
            let hash = md5_hex(uri.as_bytes());
            let filename = format!("{hash}.png");
            let _ = fs::write(fail_factory_dir.join(&filename), &tagged_bytes);
            let _ = fs::write(fail_generic_dir.join(&filename), &tagged_bytes);
        }
    }
}

/// Removes any FreeDesktop fail marker associated with `target_file`.
pub fn remove_freedesktop_fail_marker(target_file: &Path) {
    let (normal_dir, _) = freedesktop_thumbnail_dirs();
    if let Some(parent) = normal_dir.parent() {
        let uris = file_uris_for_path(target_file);
        for uri in uris {
            let hash = md5_hex(uri.as_bytes());
            let filename = format!("{hash}.png");
            let _ = fs::remove_file(
                parent
                    .join("fail")
                    .join("gnome-thumbnail-factory")
                    .join(&filename),
            );
            let _ = fs::remove_file(parent.join("fail").join(&filename));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_matches_rfc1321_vectors() {
        assert_eq!(md5_hex(b""), "d41d8cd98f00b204e9800998ecf8427e");
        assert_eq!(md5_hex(b"a"), "0cc175b9c0f1b6a831c399e269772661");
        assert_eq!(md5_hex(b"abc"), "900150983cd24fb0d6963f7d28e17f72");
        assert_eq!(
            md5_hex(b"message digest"),
            "f96b697d7cb7938d525a2f31aaf161d0"
        );
    }

    #[test]
    fn crc32_matches_standard_vector() {
        assert_eq!(crc32(b"123456789"), 0xCBF43926);
    }

    #[test]
    fn png_metadata_injection() {
        // Construct minimal valid 1x1 PNG
        let raw_png = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG magic
            0x00, 0x00, 0x00, 0x0D, // IHDR length (13)
            0x49, 0x48, 0x44, 0x52, // IHDR type
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, // 1x1 RGBA
            0x1F, 0x15, 0xC4, 0x89, // CRC
            0x00, 0x00, 0x00, 0x0A, // IDAT length (10)
            0x49, 0x44, 0x41, 0x54, // IDAT
            0x78, 0x9C, 0x63, 0x00, 0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D,
            0xB4, // data + CRC
            0x00, 0x00, 0x00, 0x00, // IEND length (0)
            0x49, 0x45, 0x4E, 0x44, // IEND
            0xAE, 0x42, 0x60, 0x82, // CRC
        ];

        let tagged = embed_freedesktop_metadata(
            &raw_png,
            "file:///home/user/photo.jpg",
            1700000000,
            "Test App",
        );
        assert!(tagged.len() > raw_png.len());
        let tagged_str = String::from_utf8_lossy(&tagged);
        assert!(tagged_str.contains("Thumb::URI"));
        assert!(tagged_str.contains("Thumb::MTime"));
        assert!(tagged_str.contains("1700000000"));
    }

    /// Encodes `img` in the requested format and returns the raw bytes.
    fn encode(img: &image::DynamicImage, format: image::ImageFormat) -> Vec<u8> {
        let mut out = std::io::Cursor::new(Vec::new());
        img.write_to(&mut out, format)
            .unwrap_or_else(|err| panic!("failed to encode {format:?}: {err}"));
        out.into_inner()
    }

    fn sample_image() -> image::DynamicImage {
        image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(48, 32, |x, y| {
            image::Rgb([(x * 5) as u8, (y * 7) as u8, 128])
        }))
    }

    /// The thumbnail cache stores PNG, so every format `ensure_png_bytes` accepts must come back
    /// as PNG. This is the regression that made GIF and WebP thumbnails silently disappear: the
    /// `image` crate was built without those decoders, `load_from_memory` failed, and the function
    /// returned the original non-PNG bytes, which the cache then served as an unreadable
    /// thumbnail.
    #[test]
    fn ensure_png_bytes_converts_every_supported_format() {
        const PNG_SIG: &[u8] = b"\x89PNG\r\n\x1a\n";

        for (label, format) in [
            ("png", image::ImageFormat::Png),
            ("jpeg", image::ImageFormat::Jpeg),
            ("gif", image::ImageFormat::Gif),
            ("webp", image::ImageFormat::WebP),
        ] {
            let encoded = encode(&sample_image(), format);
            let converted = ensure_png_bytes(&encoded);

            assert!(
                converted.starts_with(PNG_SIG),
                "{label} input was not converted to PNG — the {label} decoder is missing from the \
                 `image` feature list in the workspace Cargo.toml"
            );
            // A conversion that produced PNG-looking bytes but no decodable image is no better.
            let decoded = image::load_from_memory(&converted).unwrap_or_else(|err| {
                panic!("{label} conversion produced undecodable bytes: {err}")
            });
            assert_eq!((decoded.width(), decoded.height()), (48, 32));
        }
    }

    /// A PNG input is passed through unchanged rather than re-encoded, which keeps its metadata.
    #[test]
    fn ensure_png_bytes_passes_png_through_unchanged() {
        let encoded = encode(&sample_image(), image::ImageFormat::Png);
        assert_eq!(ensure_png_bytes(&encoded), encoded);
    }

    /// Unknown payloads must not panic or be mangled; returning them unchanged is the documented
    /// fallback for callers that pass something that is not an image at all.
    #[test]
    fn ensure_png_bytes_passes_non_image_bytes_through() {
        let garbage = b"definitely not an image".to_vec();
        assert_eq!(ensure_png_bytes(&garbage), garbage);
    }

    /// Downscaling keeps large previews within the cache's expected bounds.
    #[test]
    fn ensure_png_bytes_downscales_oversized_previews() {
        let big = image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(1200, 900, |_, _| {
            image::Rgb([10, 20, 30])
        }));
        let converted = ensure_png_bytes(&encode(&big, image::ImageFormat::Png));
        let decoded = image::load_from_memory(&converted).expect("downscaled PNG must decode");
        assert!(
            decoded.width() <= 512 && decoded.height() <= 512,
            "expected a preview within 512px, got {}x{}",
            decoded.width(),
            decoded.height()
        );
    }

    /// `freedesktop_thumbnail_is_current` is the fast path that keeps a folder of already
    /// thumbnailed files from being decoded and re-encoded again, so a version that always answered
    /// false would silently restore that cost. The cache location comes from the process
    /// environment, which this crate may not write to, so the size bookkeeping is what is pinned.
    #[test]
    fn thumbnails_count_as_current_only_when_every_size_is_present() {
        let root = std::env::temp_dir().join(format!(
            "nexia-thumb-current-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        let dirs = [
            root.join("normal"),
            root.join("large"),
            root.join("x-large"),
            root.join("xx-large"),
        ];
        for dir in &dirs {
            std::fs::create_dir_all(dir).expect("create thumbnail size directory");
        }
        let target = root.join("photo.jpg");
        std::fs::write(&target, b"not a real image").expect("write target file");

        assert!(
            !thumbnails_current_in(&dirs, &target),
            "nothing has been installed yet"
        );

        let uris = file_uris_for_path(&target);
        assert!(
            !uris.is_empty(),
            "the target must resolve to at least one URI"
        );
        for dir in &dirs {
            for uri in &uris {
                std::fs::write(
                    dir.join(format!("{}.png", md5_hex(uri.as_bytes()))),
                    b"thumb",
                )
                .expect("write thumbnail");
            }
        }
        assert!(
            thumbnails_current_in(&dirs, &target),
            "every size is installed"
        );

        // A missing size makes the entry stale: the file manager falls back to the original for
        // that size, so the honest answer is false rather than "mostly installed".
        if let Some(uri) = uris.first() {
            let _ = std::fs::remove_file(dirs[0].join(format!("{}.png", md5_hex(uri.as_bytes()))));
        }
        assert!(
            !thumbnails_current_in(&dirs, &target),
            "a missing size must report the thumbnail as stale"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
