from __future__ import annotations

from dataclasses import dataclass
import os
from pathlib import Path
import subprocess
import sys
import tempfile


class GpgError(RuntimeError):
    pass


@dataclass
class GpgDecryptResult:
    content: bytes
    filename: str | None


COMPRESSION_MAP = {
    "none": ("none", "0"),
    "zip": ("zip", "6"),
    "zlib": ("zlib", "6"),
    "bz2": ("bzip2", "6"),
}


def app_root() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys._MEIPASS)  # type: ignore[attr-defined]
    return Path(__file__).resolve().parents[1]


def gpg_binary() -> Path:
    binary = app_root() / "vendor" / "gnupg" / "bin" / "gpg.exe"
    if not binary.exists():
        raise GpgError(f"找不到内置 gpg.exe：{binary}")
    return binary


def base_env() -> dict[str, str]:
    env = os.environ.copy()
    env["PATH"] = str(gpg_binary().parent) + os.pathsep + env.get("PATH", "")
    env["LC_ALL"] = "C"
    env["LANG"] = "C"
    return env


def compression_args(name: str) -> list[str]:
    algo, level = COMPRESSION_MAP.get(name, ("zlib", "6"))
    return ["--compress-algo", algo, "-z", level]


def initialize_home(home: Path) -> None:
    home.mkdir(parents=True, exist_ok=True)
    (home / "gpg-agent.conf").write_text("allow-loopback-pinentry\n", encoding="utf-8")


def _subprocess_kwargs() -> dict:
    kwargs: dict = {}
    if sys.platform == "win32":
        kwargs["creationflags"] = subprocess.CREATE_NO_WINDOW
    return kwargs


def run_gpg(homedir: Path, args: list[str], input_bytes: bytes | None = None, check: bool = True) -> subprocess.CompletedProcess:
    command = [
        str(gpg_binary()),
        "--homedir",
        str(homedir),
        "--batch",
        "--yes",
        "--no-tty",
        "--status-fd",
        "2",
        *args,
    ]
    completed = subprocess.run(
        command,
        input=input_bytes,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        env=base_env(),
        check=False,
        **_subprocess_kwargs(),
    )
    if check and completed.returncode != 0:
        stderr = completed.stderr.decode("utf-8", errors="replace")
        message = next((line for line in stderr.splitlines() if line and not line.startswith("[GNUPG:]")), stderr.strip())
        raise GpgError(message or f"gpg 执行失败，退出码 {completed.returncode}")
    return completed


def import_key(homedir: Path, key_text: str) -> None:
    run_gpg(homedir, ["--import"], input_bytes=key_text.encode("utf-8"))


def parse_plaintext_filename(stderr: bytes) -> str | None:
    for line in stderr.decode("utf-8", errors="replace").splitlines():
        if not line.startswith("[GNUPG:] PLAINTEXT "):
            continue
        parts = line.split(" ", 4)
        if len(parts) < 5:
            return None
        filename = parts[4].strip()
        if not filename or filename in {"_CONSOLE", "[none]"}:
            return None
        return filename
    return None


def encrypt_bytes(
    *,
    payload: bytes,
    armor: bool,
    compression_name: str,
    mode: str,
    passphrase: str = "",
    source_filename: str | None = None,
    public_key_text: str | None = None,
    public_key_fingerprint: str | None = None,
) -> bytes:
    with tempfile.TemporaryDirectory() as work_dir:
        work_path = Path(work_dir)
        home = work_path / "gnupg-home"
        initialize_home(home)

        output_path = work_path / ("output.asc" if armor else "output.pgp")
        args: list[str] = []

        if armor:
            args.append("--armor")
        args.extend(compression_args(compression_name))

        input_bytes: bytes | None = None
        if source_filename:
            input_path = work_path / source_filename
            input_path.write_bytes(payload)
        else:
            input_path = None
            input_bytes = payload

        if mode == "symmetric":
            args.extend(
                [
                    "--pinentry-mode",
                    "loopback",
                    "--passphrase",
                    passphrase,
                    "--symmetric",
                    "--cipher-algo",
                    "AES256",
                ]
            )
        elif mode == "public":
            if not public_key_text or not public_key_fingerprint:
                raise GpgError("公钥模式缺少密钥信息。")
            import_key(home, public_key_text)
            args.extend(
                [
                    "--trust-model",
                    "always",
                    "--recipient",
                    public_key_fingerprint,
                    "--encrypt",
                ]
            )
        else:
            raise GpgError("不支持的加密模式。")

        args.extend(["--output", str(output_path)])
        if input_path is not None:
            args.append(str(input_path))

        run_gpg(home, args, input_bytes=input_bytes)
        return output_path.read_bytes()


def decrypt_bytes(
    *,
    encrypted_blob: str | bytes,
    mode: str,
    passphrase: str = "",
    private_key_text: str | None = None,
) -> GpgDecryptResult:
    blob_bytes = encrypted_blob.encode("utf-8") if isinstance(encrypted_blob, str) else encrypted_blob

    with tempfile.TemporaryDirectory() as work_dir:
        work_path = Path(work_dir)
        home = work_path / "gnupg-home"
        initialize_home(home)

        if mode == "public":
            if not private_key_text:
                raise GpgError("私钥模式缺少密钥信息。")
            import_key(home, private_key_text)

        input_path = work_path / "cipher_input.bin"
        input_path.write_bytes(blob_bytes)

        args: list[str] = ["--output", "-", "--decrypt", str(input_path)]
        if mode in {"symmetric", "public"}:
            args = [
                "--pinentry-mode",
                "loopback",
                "--passphrase",
                passphrase,
                *args,
            ]

        completed = run_gpg(home, args, check=False)
        if completed.returncode != 0:
            stderr = completed.stderr.decode("utf-8", errors="replace")
            message = next((line for line in stderr.splitlines() if line and not line.startswith("[GNUPG:]")), stderr.strip())
            raise GpgError(message or f"gpg 解密失败，退出码 {completed.returncode}")

        return GpgDecryptResult(
            content=completed.stdout,
            filename=parse_plaintext_filename(completed.stderr),
        )
