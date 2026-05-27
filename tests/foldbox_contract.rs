use foldbox::app::{
    PackRequest, default_pack_output_path, default_unpack_output_path, pack_bytes, unpack_text,
};
use tempfile::tempdir;

#[test]
fn pack_and_unpack_roundtrip_preserves_filename_and_bytes() {
    let encoded = pack_bytes(PackRequest {
        input_name: "demo.7z",
        input_bytes: b"hello foldbox".to_vec(),
        passphrase: "secret",
        confirm_passphrase: Some("secret"),
    })
    .unwrap();

    let unpacked = unpack_text(&encoded, "secret").unwrap();
    assert_eq!(unpacked.file_name, "demo.7z");
    assert_eq!(unpacked.file_bytes, b"hello foldbox");
}

#[test]
fn unpack_ignores_all_whitespace() {
    let encoded = pack_bytes(PackRequest {
        input_name: "archive.bin",
        input_bytes: (0_u8..32).collect(),
        passphrase: "secret",
        confirm_passphrase: Some("secret"),
    })
    .unwrap();

    let decorated = encoded
        .chars()
        .enumerate()
        .flat_map(|(index, character)| {
            let mut chunk = String::new();
            chunk.push(character);
            if index % 11 == 0 {
                chunk.push(' ');
            }
            if index % 17 == 0 {
                chunk.push('\t');
            }
            if index % 23 == 0 {
                chunk.push('\n');
            }
            chunk.chars().collect::<Vec<_>>()
        })
        .collect::<String>();

    let unpacked = unpack_text(&decorated, "secret").unwrap();
    assert_eq!(unpacked.file_name, "archive.bin");
    assert_eq!(unpacked.file_bytes, (0_u8..32).collect::<Vec<_>>());
}

#[test]
fn wrong_passphrase_returns_generic_failure() {
    let encoded = pack_bytes(PackRequest {
        input_name: "demo.bin",
        input_bytes: b"top secret".to_vec(),
        passphrase: "secret",
        confirm_passphrase: Some("secret"),
    })
    .unwrap();

    let error = unpack_text(&encoded, "wrong").unwrap_err();
    assert_eq!(error.to_string(), "处理失败。");
}

#[test]
fn pack_rejects_mismatched_confirmation() {
    let error = pack_bytes(PackRequest {
        input_name: "demo.bin",
        input_bytes: b"hello".to_vec(),
        passphrase: "secret",
        confirm_passphrase: Some("mismatch"),
    })
    .unwrap_err();

    assert_eq!(error.to_string(), "两次口令不一致。");
}

#[test]
fn default_pack_output_uses_box_txt_suffix() {
    let input = std::path::Path::new("C:/work/demo.7z");
    let path = default_pack_output_path(input);
    assert_eq!(path, input.with_file_name("demo.7z.box.txt"));
}

#[test]
fn default_unpack_output_restores_original_filename_in_same_directory() {
    let dir = tempdir().unwrap();
    let encoded_path = dir.path().join("demo.box.txt");
    let output_path = default_unpack_output_path(&encoded_path, "payload.bin");
    assert_eq!(output_path, dir.path().join("payload.bin"));
}
