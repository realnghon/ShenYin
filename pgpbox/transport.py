from __future__ import annotations


INLINE_TEXT_THRESHOLD = 100_000


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
        return blob.strip()
    return blob
