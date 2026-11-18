//! File and stream byte budgets. These bound input buffers, not decoded heap
//! size, CPU time, child processes or total RSS.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Component, Path};

use anyhow::{bail, Context, Result};

/// Repository policy defaults, not Stellar protocol limits.
pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_ARTIFACT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_REPORT_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_DECOMPRESSED_BYTES: u64 = 256 * 1024 * 1024;

/// Read at most `max + 1` bytes, including when metadata is unavailable or
/// a file grows after opening. The extra byte distinguishes EOF from overflow.
pub fn read_bounded(reader: impl Read, max: u64) -> Result<Vec<u8>> {
    let limit = max.checked_add(1).context("input byte limit overflow")?;
    let mut bytes = Vec::new();
    reader.take(limit).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > max {
        bail!("input exceeds {max} bytes");
    }
    Ok(bytes)
}

/// Read a regular, non-symlink file within a byte budget. Files are assumed
/// stable during the operation; this does not sandbox concurrent path changes.
pub fn read_file(path: &Path, max: u64) -> Result<Vec<u8>> {
    let read = || -> Result<Vec<u8>> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.file_type().is_file() {
            bail!("input must be a regular file, not a symlink or directory");
        }
        if metadata.len() > max {
            bail!(
                "input exceeds {max} bytes (file has {} bytes)",
                metadata.len()
            );
        }
        let file = File::open(path)?;
        if !file.metadata()?.is_file() {
            bail!("opened input is not a regular file");
        }
        read_bounded(file, max)
    };
    read().with_context(|| format!("reading {}", path.display()))
}

/// Manifest artifacts must be regular files immediately inside their entry.
pub fn read_artifact(dir: &Path, name: &str) -> Result<Vec<u8>> {
    let mut components = Path::new(name).components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        bail!("artifact path must be a filename inside the entry directory");
    }
    read_file(&dir.join(name), MAX_ARTIFACT_BYTES)
}

pub fn decompress_bounded(compressed: &[u8], max: u64) -> Result<Vec<u8>> {
    read_bounded(zstd::stream::read::Decoder::new(compressed)?, max)
        .context("decompressing data-lake object")
}
