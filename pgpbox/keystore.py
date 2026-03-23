from __future__ import annotations

from dataclasses import dataclass
from pathlib import Path
from typing import Any

from pgpy import PGPKey

from pgpbox.crypto import CryptoError, unwrap_pgpy


class KeyStoreError(CryptoError):
    pass


@dataclass
class KeyAsset:
    filename: str
    path: Path


class KeyStore:
    def __init__(self, base_dir: Path):
        self.base_dir = base_dir
        self.public_dir = self.base_dir / "public"
        self.private_dir = self.base_dir / "private"
        self.public_dir.mkdir(parents=True, exist_ok=True)
        self.private_dir.mkdir(parents=True, exist_ok=True)

    @staticmethod
    def normalize_fingerprint(fingerprint: str) -> str:
        return "".join(ch for ch in fingerprint.upper() if ch.isalnum())

    def _dir_for_kind(self, kind: str) -> Path:
        if kind == "public":
            return self.public_dir
        if kind == "private":
            return self.private_dir
        raise KeyStoreError("密钥类型只能是 public 或 private。")

    def _path_for(self, kind: str, fingerprint: str) -> Path:
        normalized = self.normalize_fingerprint(fingerprint)
        if not normalized:
            raise KeyStoreError("缺少有效的密钥指纹。")
        return self._dir_for_kind(kind) / f"{normalized}.asc"

    def _format_userids(self, key: PGPKey) -> list[str]:
        labels: list[str] = []
        for uid in key.userids:
            parts = []
            if uid.name:
                parts.append(uid.name)
            if uid.email:
                parts.append(f"<{uid.email}>")
            if uid.comment:
                parts.append(f"({uid.comment})")
            labels.append(" ".join(parts) if parts else "Unnamed UID")
        return labels or ["No user ID"]

    def _summary(self, key: PGPKey, kind: str) -> dict[str, Any]:
        fingerprint = self.normalize_fingerprint(str(key.fingerprint))
        userids = self._format_userids(key)
        return {
            "kind": kind,
            "fingerprint": fingerprint,
            "display_fingerprint": str(key.fingerprint),
            "key_id": key.fingerprint.keyid,
            "label": userids[0],
            "user_ids": userids,
            "is_protected": bool(getattr(key, "is_protected", False)),
        }

    def _parse_blob(self, blob: str | bytes) -> PGPKey:
        try:
            return unwrap_pgpy(PGPKey.from_blob(blob))
        except Exception as exc:  # noqa: BLE001
            raise KeyStoreError(f"密钥导入失败：{exc}") from exc

    def _load_path(self, path: Path) -> PGPKey:
        if not path.exists():
            raise KeyStoreError("找不到对应的密钥文件。")
        return self._parse_blob(path.read_text(encoding="utf-8"))

    def save_key(self, key: PGPKey) -> list[dict[str, Any]]:
        saved: list[dict[str, Any]] = []
        if key.is_public:
            saved.append(self._write_key(key, "public"))
            return saved

        saved.append(self._write_key(key, "private"))
        saved.append(self._write_key(key.pubkey, "public"))
        return saved

    def _write_key(self, key: PGPKey, kind: str) -> dict[str, Any]:
        path = self._path_for(kind, str(key.fingerprint))
        path.write_text(str(key), encoding="utf-8")
        return self._summary(key, kind)

    def import_blob(self, blob: str | bytes) -> list[dict[str, Any]]:
        key = self._parse_blob(blob)
        return self.save_key(key)

    def list_keys(self, kind: str) -> list[dict[str, Any]]:
        items: list[dict[str, Any]] = []
        for path in sorted(self._dir_for_kind(kind).glob("*.asc")):
            try:
                key = self._load_path(path)
            except KeyStoreError:
                continue
            items.append(self._summary(key, kind))
        return items

    def list_all(self) -> dict[str, list[dict[str, Any]]]:
        return {
            "public": self.list_keys("public"),
            "private": self.list_keys("private"),
        }

    def load_key(self, kind: str, fingerprint: str) -> PGPKey:
        return self._load_path(self._path_for(kind, fingerprint))

    def export_key(self, kind: str, fingerprint: str) -> KeyAsset:
        path = self._path_for(kind, fingerprint)
        if not path.exists():
            raise KeyStoreError("要下载的密钥不存在。")
        suffix = "public" if kind == "public" else "private"
        return KeyAsset(filename=f"{path.stem}.{suffix}.asc", path=path)

    def delete_key(self, kind: str, fingerprint: str) -> None:
        path = self._path_for(kind, fingerprint)
        if not path.exists():
            raise KeyStoreError("要删除的密钥不存在。")
        path.unlink()
