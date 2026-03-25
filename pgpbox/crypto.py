from __future__ import annotations

from dataclasses import dataclass

from werkzeug.utils import secure_filename

from pgpbox.engine import EngineError, decrypt_bytes, encrypt_bytes
from pgpbox.transport import encode_text_output, normalize_transport_blob


class CryptoError(ValueError):
    pass


@dataclass
class CryptoResult:
    kind: str
    filename: str
    content: bytes
    mime_type: str
    inline_text: str | None = None


def safe_filename(name: str, fallback: str) -> str:
    cleaned = secure_filename(name)
    return cleaned or fallback


def _download_name(input_type: str, source_name: str | None, text_mode: bool) -> str:
    base_name = source_name if input_type == "file" and source_name else "message"
    extension = "txt" if text_mode else "bin"
    return f"{base_name}.{extension}"


def encrypt_content(
    *,
    input_type: str,
    armor: bool,
    compression_name: str,
    passphrase: str = "",
    text_value: str = "",
    file_name: str | None = None,
    file_bytes: bytes | None = None,
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

    if not passphrase:
        raise CryptoError("必须输入访问码。")

    try:
        output_bytes = encrypt_bytes(
            payload=payload,
            passphrase=passphrase,
            compression_name=compression_name,
            source_filename=source_name if input_type == "file" else None,
        )
    except EngineError as exc:
        raise CryptoError(f"加密失败：{exc}") from exc

    if armor:
        inline_text = encode_text_output(output_bytes)
        text_bytes = inline_text.encode("utf-8")
        filename = _download_name(input_type, source_name, True)
        return CryptoResult(
            kind="text",
            filename=filename,
            content=text_bytes,
            mime_type="text/plain; charset=utf-8",
            inline_text=inline_text,
        )

    filename = _download_name(input_type, source_name, False)
    return CryptoResult(
        kind="download",
        filename=filename,
        content=output_bytes,
        mime_type="application/octet-stream",
    )


def decrypt_content(
    *,
    encrypted_blob: str | bytes,
    passphrase: str = "",
) -> CryptoResult:
    if not encrypted_blob:
        raise CryptoError("请提供要解密的内容。")
    if not passphrase:
        raise CryptoError("必须输入访问码。")

    normalized_blob = normalize_transport_blob(encrypted_blob)

    try:
        decrypted = decrypt_bytes(
            encrypted_blob=normalized_blob,
            passphrase=passphrase,
        )
    except EngineError as exc:
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
