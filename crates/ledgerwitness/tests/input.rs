use std::fs;
use std::io::Cursor;

use ledgerwitness::input::{
    decompress_bounded, read_artifact, read_bounded, read_file, MAX_ARTIFACT_BYTES,
};

#[test]
fn exact_byte_limit_and_empty_input_are_accepted() {
    assert_eq!(read_bounded(Cursor::new(b"1234"), 4).unwrap(), b"1234");
    assert!(read_bounded(Cursor::new(b""), 0).unwrap().is_empty());
}

#[test]
fn overflow_reads_only_one_byte_past_budget() {
    let mut input = Cursor::new([0; 100]);
    assert!(read_bounded(&mut input, 8).is_err());
    assert_eq!(input.position(), 9);
    assert!(read_bounded(Cursor::new([]), u64::MAX).is_err());
}

#[test]
fn sparse_oversized_file_is_rejected_without_loading_it() {
    let path = std::env::temp_dir().join(format!(
        "ledgerwitness-input-{}-oversized",
        std::process::id()
    ));
    let file = fs::File::create(&path).unwrap();
    file.set_len(MAX_ARTIFACT_BYTES + 1).unwrap();
    let error = read_file(&path, MAX_ARTIFACT_BYTES).unwrap_err();
    assert!(format!("{error:#}").contains("exceeds"));
    fs::remove_file(path).unwrap();
}

#[test]
fn decompression_bomb_stops_at_output_budget() {
    let compressed = zstd::stream::encode_all(Cursor::new([0; 4096]), 1).unwrap();
    assert_eq!(decompress_bounded(&compressed, 4096).unwrap().len(), 4096);
    assert!(decompress_bounded(&compressed, 64).is_err());
    assert!(decompress_bounded(b"not-zstd", 64).is_err());
}

#[test]
fn parent_and_absolute_artifact_paths_are_rejected() {
    for name in ["../escape", "/escape", ".", "..", ""] {
        assert!(read_artifact(std::env::temp_dir().as_path(), name).is_err());
    }
}

#[cfg(unix)]
#[test]
fn symlinks_and_directories_are_rejected() {
    let dir = std::env::temp_dir().join(format!(
        "ledgerwitness-input-{}-symlink",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("real"), b"ok").unwrap();
    let link = dir.join("link");
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(dir.join("real"), &link).unwrap();
    assert!(read_file(&link, 64).is_err());
    assert!(read_file(&dir, 64).is_err());
    fs::remove_dir_all(dir).unwrap();
}
