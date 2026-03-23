from __future__ import annotations

from dataclasses import dataclass
import json
from pathlib import Path
import shutil
import time
import uuid


@dataclass
class StoredResult:
    token: str
    filename: str
    mime_type: str
    size: int
    path: Path


class ResultStore:
    def __init__(self, base_dir: Path, ttl_hours: int = 24):
        self.base_dir = base_dir
        self.ttl_hours = ttl_hours
        self.base_dir.mkdir(parents=True, exist_ok=True)
        self.cleanup()

    def _token_dir(self, token: str) -> Path:
        return self.base_dir / token

    def _meta_path(self, token: str) -> Path:
        return self._token_dir(token) / "meta.json"

    def _payload_path(self, token: str) -> Path:
        return self._token_dir(token) / "payload.bin"

    def save(self, content: bytes, filename: str, mime_type: str) -> StoredResult:
        token = uuid.uuid4().hex
        token_dir = self._token_dir(token)
        token_dir.mkdir(parents=True, exist_ok=True)

        payload_path = self._payload_path(token)
        payload_path.write_bytes(content)

        meta = {
            "filename": filename,
            "mime_type": mime_type,
            "size": len(content),
            "created_at": time.time(),
        }
        self._meta_path(token).write_text(json.dumps(meta, ensure_ascii=False), encoding="utf-8")

        return StoredResult(
            token=token,
            filename=filename,
            mime_type=mime_type,
            size=len(content),
            path=payload_path,
        )

    def get(self, token: str) -> StoredResult:
        meta_path = self._meta_path(token)
        payload_path = self._payload_path(token)
        if not meta_path.exists() or not payload_path.exists():
            raise FileNotFoundError("下载内容已不存在。")

        meta = json.loads(meta_path.read_text(encoding="utf-8"))
        return StoredResult(
            token=token,
            filename=meta["filename"],
            mime_type=meta["mime_type"],
            size=meta["size"],
            path=payload_path,
        )

    def cleanup(self) -> None:
        cutoff = time.time() - (self.ttl_hours * 3600)
        for path in self.base_dir.iterdir():
            if not path.is_dir():
                continue
            meta_path = path / "meta.json"
            if not meta_path.exists():
                shutil.rmtree(path, ignore_errors=True)
                continue
            try:
                meta = json.loads(meta_path.read_text(encoding="utf-8"))
            except json.JSONDecodeError:
                shutil.rmtree(path, ignore_errors=True)
                continue
            if meta.get("created_at", 0) < cutoff:
                shutil.rmtree(path, ignore_errors=True)
