from __future__ import annotations

from dataclasses import dataclass

from pgpy import PGPKey, PGPUID
from pgpy.constants import CompressionAlgorithm, HashAlgorithm, KeyFlags, PubKeyAlgorithm, SymmetricKeyAlgorithm
from werkzeug.utils import secure_filename

from pgpbox.gpg_engine import GpgError, decrypt_bytes, encrypt_bytes
from pgpbox.transport import normalize_transport_blob


class CryptoError(ValueError):
    pass


@dataclass
class CryptoResult:
    kind: str
    filename: str
    content: bytes
    mime_type: str
    inline_text: str | None = None


def unwrap_pgpy(value):
    if isinstance(value, tuple):
        return value[0]
    if isinstance(value, list):
        return value[0]
    return value


def safe_filename(name: str, fallback: str) -> str:
    cleaned = secure_filename(name)
    return cleaned or fallback


def generate_rsa_key(
    *,
    name: str,
    email: str,
    comment: str,
    passphrase: str,
    key_size: int,
) -> PGPKey:
    if not name:
        raise CryptoError("生成密钥时必须填写姓名。")
    if not email:
        raise CryptoError("生成密钥时必须填写邮箱。")
    if key_size not in {2048, 3072, 4096}:
        raise CryptoError("密钥位数仅支持 2048 / 3072 / 4096。")

    key = PGPKey.new(PubKeyAlgorithm.RSAEncryptOrSign, key_size)
    uid = PGPUID.new(name, comment=comment, email=email)
    key.add_uid(
        uid,
        usage={
            KeyFlags.Certify,
            KeyFlags.Sign,
            KeyFlags.EncryptCommunications,
            KeyFlags.EncryptStorage,
        },
        hashes=[HashAlgorithm.SHA256, HashAlgorithm.SHA384, HashAlgorithm.SHA512],
        ciphers=[SymmetricKeyAlgorithm.AES256, SymmetricKeyAlgorithm.AES192, SymmetricKeyAlgorithm.AES128],
        compression=[
            CompressionAlgorithm.ZLIB,
            CompressionAlgorithm.ZIP,
            CompressionAlgorithm.Uncompressed,
        ],
    )

    if passphrase:
        key.protect(passphrase, SymmetricKeyAlgorithm.AES256, HashAlgorithm.SHA256)

    return key


def _download_name(input_type: str, source_name: str | None, armor: bool) -> str:
    base_name = source_name if input_type == "file" and source_name else "message"
    extension = "asc" if armor else "pgp"
    return f"{base_name}.{extension}"


def encrypt_content(
    *,
    input_type: str,
    mode: str,
    armor: bool,
    compression_name: str,
    passphrase: str = "",
    text_value: str = "",
    file_name: str | None = None,
    file_bytes: bytes | None = None,
    public_key: PGPKey | None = None,
    public_key_fingerprint: str | None = None,
) -> CryptoResult:
    if input_type == "text":
        if not text_value:
            raise CryptoError("请输入要处理的纯文本。")
        payload = text_value.encode("utf-8")
        source_name = None
    elif input_type == "file":
        if file_name is None or file_bytes is None:
            raise CryptoError("文件模式下必须提供文件名和内容。")
        if not file_bytes:
            raise CryptoError("文件内容为空，无法继续。")
        payload = file_bytes
        source_name = safe_filename(file_name, "payload.bin")
    else:
        raise CryptoError("不支持的输入类型。")

    if mode == "symmetric" and not passphrase:
        raise CryptoError("对称加密必须输入口令。")
    if mode == "public" and public_key is None:
        raise CryptoError("请选择一个公钥。")

    try:
        output_bytes = encrypt_bytes(
            payload=payload,
            armor=armor,
            compression_name=compression_name,
            mode=mode,
            passphrase=passphrase,
            source_filename=source_name if input_type == "file" else None,
            public_key_text=str(public_key) if public_key is not None else None,
            public_key_fingerprint=public_key_fingerprint or (str(public_key.fingerprint) if public_key is not None else None),
        )
    except GpgError as exc:
        raise CryptoError(f"加密失败：{exc}") from exc

    filename = _download_name(input_type, source_name, armor)
    mime_type = "text/plain; charset=utf-8" if armor else "application/octet-stream"
    inline_text = output_bytes.decode("utf-8") if armor else None

    return CryptoResult(
        kind="text" if inline_text is not None else "download",
        filename=filename,
        content=output_bytes,
        mime_type=mime_type,
        inline_text=inline_text,
    )


def decrypt_content(
    *,
    mode: str,
    encrypted_blob: str | bytes,
    passphrase: str = "",
    private_key: PGPKey | None = None,
) -> CryptoResult:
    if not encrypted_blob:
        raise CryptoError("请提供要解密的内容。")
    if mode == "symmetric" and not passphrase:
        raise CryptoError("对称解密必须输入口令。")
    if mode == "public" and private_key is None:
        raise CryptoError("请选择一个私钥。")

    normalized_blob = normalize_transport_blob(encrypted_blob)

    try:
        decrypted = decrypt_bytes(
            encrypted_blob=normalized_blob,
            mode=mode,
            passphrase=passphrase,
            private_key_text=str(private_key) if private_key is not None else None,
        )
    except GpgError as exc:
        raise CryptoError(f"解密失败：{exc}") from exc

    if decrypted.filename:
        filename = safe_filename(decrypted.filename, "decrypted.bin")
        return CryptoResult(
            kind="download",
            filename=filename,
            content=decrypted.content,
            mime_type="application/octet-stream",
        )

    try:
        text_value = decrypted.content.decode("utf-8")
    except UnicodeDecodeError:
        return CryptoResult(
            kind="download",
            filename="decrypted.bin",
            content=decrypted.content,
            mime_type="application/octet-stream",
        )

    return CryptoResult(
        kind="text",
        filename="decrypted.txt",
        content=text_value.encode("utf-8"),
        mime_type="text/plain; charset=utf-8",
        inline_text=text_value,
    )
