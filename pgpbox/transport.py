from __future__ import annotations

import base64
import textwrap


INLINE_TEXT_THRESHOLD = 100_000
_LINE_WIDTH = 76


def encode_text_output(raw_bytes: bytes) -> str:
    encoded = base64.b85encode(raw_bytes).decode("ascii")
    return "\n".join(textwrap.wrap(encoded, _LINE_WIDTH))


def decode_text_input(text: str) -> bytes:
    stripped = text.replace("\n", "").replace("\r", "").strip()
    return base64.b85decode(stripped)


def is_pgp_armor(blob: str) -> bool:
    return blob.lstrip().startswith("-----BEGIN PGP ")


def extract_text_payload(text: str, filename: str, token: str) -> dict[str, object]:
    del filename, token

    if len(text) > INLINE_TEXT_THRESHOLD:
        return {
            "text_available": False,
            "text_too_large": True,
            "text_length": len(text),
        }

    return {
        "text_available": True,
        "text_too_large": False,
        "text_length": len(text),
        "text": text,
    }


def normalize_transport_blob(blob: str | bytes) -> str | bytes:
    if isinstance(blob, str):
        text = blob.strip()
        if is_pgp_armor(text):
            return text
        try:
            return decode_text_input(text)
        except Exception:
            return text

    try:
        text = blob.decode("utf-8").strip()
    except UnicodeDecodeError:
        return blob

    if is_pgp_armor(text):
        return text

    try:
        return decode_text_input(text)
    except Exception:
        return blob
