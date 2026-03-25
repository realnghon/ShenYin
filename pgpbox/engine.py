"""Pure-Python AES-256-GCM encryption engine.

No subprocess calls, no temp directories, no external binaries.
Uses PBKDF2-HMAC-SHA256 for key derivation and AES-256-GCM for
authenticated encryption.  Works identically on every Windows machine.
"""

from __future__ import annotations

import bz2
import json
import os
import struct
import zlib
from dataclasses import dataclass

from cryptography.hazmat.primitives.ciphers.aead import AESGCM
from cryptography.hazmat.primitives.kdf.pbkdf2 import PBKDF2HMAC
from cryptography.hazmat.primitives import hashes


class EngineError(RuntimeError):
    pass


@dataclass
class DecryptResult:
    content: bytes
    filename: str | None


_VERSION = 1
_SALT_LEN = 16
_NONCE_LEN = 12
_ITERATIONS = 600_000
_KEY_LEN = 32  # AES-256


def _derive_key(passphrase: str, salt: bytes) -> bytes:
    kdf = PBKDF2HMAC(
        algorithm=hashes.SHA256(),
        length=_KEY_LEN,
        salt=salt,
        iterations=_ITERATIONS,
    )
    return kdf.derive(passphrase.encode("utf-8"))


def _compress(data: bytes, algo: str) -> bytes:
    if algo in ("zlib", "zip"):
        return zlib.compress(data, 6)
    if algo == "bz2":
        return bz2.compress(data, 6)
    return data


def _decompress(data: bytes, algo: str) -> bytes:
    if algo in ("zlib", "zip"):
        return zlib.decompress(data)
    if algo == "bz2":
        return bz2.decompress(data)
    return data


def encrypt_bytes(
    *,
    payload: bytes,
    passphrase: str,
    compression_name: str = "zlib",
    source_filename: str | None = None,
) -> bytes:
    meta: dict = {}
    if source_filename:
        meta["f"] = source_filename
    if compression_name and compression_name != "none":
        meta["c"] = compression_name

    meta_bytes = json.dumps(meta, ensure_ascii=False, separators=(",", ":")).encode("utf-8")

    compressed = _compress(payload, compression_name)

    salt = os.urandom(_SALT_LEN)
    nonce = os.urandom(_NONCE_LEN)
    key = _derive_key(passphrase, salt)
    aesgcm = AESGCM(key)
    ciphertext = aesgcm.encrypt(nonce, compressed, meta_bytes)

    return (
        struct.pack("B", _VERSION)
        + salt
        + nonce
        + struct.pack(">I", len(meta_bytes))
        + meta_bytes
        + ciphertext
    )


def decrypt_bytes(
    *,
    encrypted_blob: bytes,
    passphrase: str,
) -> DecryptResult:
    min_len = 1 + _SALT_LEN + _NONCE_LEN + 4
    if len(encrypted_blob) < min_len:
        raise EngineError("数据格式无效或已损坏。")

    offset = 0
    version = encrypted_blob[offset]
    offset += 1

    if version != _VERSION:
        raise EngineError(f"不支持的数据版本：{version}")

    salt = encrypted_blob[offset : offset + _SALT_LEN]
    offset += _SALT_LEN

    nonce = encrypted_blob[offset : offset + _NONCE_LEN]
    offset += _NONCE_LEN

    (meta_len,) = struct.unpack(">I", encrypted_blob[offset : offset + 4])
    offset += 4

    if len(encrypted_blob) < offset + meta_len:
        raise EngineError("数据格式无效或已损坏。")

    meta_bytes = encrypted_blob[offset : offset + meta_len]
    offset += meta_len

    ciphertext = encrypted_blob[offset:]

    key = _derive_key(passphrase, salt)
    aesgcm = AESGCM(key)

    try:
        compressed = aesgcm.decrypt(nonce, ciphertext, meta_bytes)
    except Exception:
        raise EngineError("解密失败：访问码不正确或数据已损坏。")

    try:
        meta = json.loads(meta_bytes.decode("utf-8"))
    except (json.JSONDecodeError, UnicodeDecodeError):
        meta = {}

    compression = meta.get("c", "none")
    try:
        plaintext = _decompress(compressed, compression)
    except Exception:
        raise EngineError("解压缩失败：数据可能已损坏。")

    return DecryptResult(
        content=plaintext,
        filename=meta.get("f"),
    )
