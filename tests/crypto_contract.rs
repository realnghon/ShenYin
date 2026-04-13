use shenyin::crypto::{EncryptRequest, decrypt_content, encrypt_content};

fn text_request(text: &str, compression: &str) -> EncryptRequest {
    EncryptRequest {
        input_type: "text".into(),
        armor: true,
        compression_name: compression.into(),
        passphrase: "secret".into(),
        text_value: text.into(),
        file_name: None,
        file_bytes: None,
    }
}

#[test]
fn symmetric_text_roundtrip() {
    let encrypted = encrypt_content(text_request("hello armored world", "zlib")).unwrap();
    assert_eq!(encrypted.kind, "text");
    assert!(
        encrypted
            .inline_text
            .as_ref()
            .unwrap()
            .chars()
            .all(|character| character.is_ascii_graphic() || character == '\n' || character == '\r')
    );

    let decrypted = decrypt_content(
        encrypted.inline_text.as_ref().unwrap().as_bytes(),
        "secret",
    )
    .unwrap();
    assert_eq!(decrypted.kind, "text");
    assert_eq!(decrypted.inline_text.as_deref(), Some("hello armored world"));
}

#[test]
fn symmetric_file_roundtrip() {
    let encrypted = encrypt_content(EncryptRequest {
        input_type: "file".into(),
        armor: false,
        compression_name: "zip".into(),
        passphrase: "secret".into(),
        text_value: String::new(),
        file_name: Some("archive.zip".into()),
        file_bytes: Some((0_u8..64).collect()),
    })
    .unwrap();

    let decrypted = decrypt_content(&encrypted.content, "secret").unwrap();
    assert_eq!(decrypted.kind, "download");
    assert_eq!(decrypted.filename, "archive.zip");
    assert_eq!(decrypted.content, (0_u8..64).collect::<Vec<_>>());
}

#[test]
fn symmetric_file_to_text_roundtrip() {
    let encrypted = encrypt_content(EncryptRequest {
        input_type: "file".into(),
        armor: true,
        compression_name: "zlib".into(),
        passphrase: "secret".into(),
        text_value: String::new(),
        file_name: Some("notes.bin".into()),
        file_bytes: Some(b"\x00\x01\x02hello".to_vec()),
    })
    .unwrap();

    let decrypted = decrypt_content(
        encrypted.inline_text.as_ref().unwrap().as_bytes(),
        "secret",
    )
    .unwrap();
    assert_eq!(decrypted.kind, "download");
    assert_eq!(decrypted.filename, "notes.bin");
    assert_eq!(decrypted.content, b"\x00\x01\x02hello");
}

#[test]
fn wrong_passphrase_raises() {
    let encrypted = encrypt_content(text_request("secret data", "zlib")).unwrap();
    let error = decrypt_content(
        encrypted.inline_text.as_ref().unwrap().as_bytes(),
        "wrong",
    )
    .unwrap_err();
    assert_eq!(error.to_string(), "解密失败：访问码不正确或数据已损坏。");
}

#[test]
fn bz2_roundtrip() {
    let encrypted = encrypt_content(text_request("hello bz2", "bz2")).unwrap();
    let decrypted = decrypt_content(
        encrypted.inline_text.as_ref().unwrap().as_bytes(),
        "secret",
    )
    .unwrap();
    assert_eq!(decrypted.inline_text.as_deref(), Some("hello bz2"));
}

#[test]
fn no_compression_roundtrip() {
    let encrypted = encrypt_content(text_request("hello none", "none")).unwrap();
    let decrypted = decrypt_content(
        encrypted.inline_text.as_ref().unwrap().as_bytes(),
        "secret",
    )
    .unwrap();
    assert_eq!(decrypted.inline_text.as_deref(), Some("hello none"));
}
