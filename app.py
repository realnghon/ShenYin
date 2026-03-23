from __future__ import annotations

import argparse
import sys
import threading
import webbrowser
from pathlib import Path

from flask import Flask, jsonify, render_template, request, send_file, url_for
from werkzeug.exceptions import HTTPException, RequestEntityTooLarge

from pgpbox.crypto import CryptoError, decrypt_content, encrypt_content, generate_rsa_key
from pgpbox.keystore import KeyStore, KeyStoreError
from pgpbox.result_store import ResultStore
from pgpbox.transport import extract_text_payload


def app_root() -> Path:
    if getattr(sys, "frozen", False):
        return Path(sys._MEIPASS)  # type: ignore[attr-defined]
    return Path(__file__).resolve().parent


APP_ROOT = app_root()
RUNTIME_ROOT = Path(sys.executable).resolve().parent if getattr(sys, "frozen", False) else APP_ROOT
DATA_DIR = RUNTIME_ROOT / "data"
KEY_STORE = KeyStore(DATA_DIR / "keys")
RESULT_STORE = ResultStore(DATA_DIR / "results")

app = Flask(
    __name__,
    template_folder=str(APP_ROOT / "templates"),
    static_folder=str(APP_ROOT / "static"),
)
app.config["MAX_CONTENT_LENGTH"] = 1024 * 1024 * 1024
app.config["MAX_FORM_MEMORY_SIZE"] = 1024 * 1024 * 1024
app.json.sort_keys = False


def json_error(message: str, status_code: int = 400):
    return jsonify({"ok": False, "message": message}), status_code


@app.errorhandler(RequestEntityTooLarge)
def handle_file_too_large(exc):
    if request.path.startswith("/api/"):
        return json_error("文件过大，超出了当前工具允许的大小。", 413)
    return str(exc), 413


@app.errorhandler(Exception)
def handle_unexpected_error(exc):
    if isinstance(exc, HTTPException):
        if request.path.startswith("/api/"):
            return json_error(exc.description, exc.code or 500)
        return exc
    if request.path.startswith("/api/"):
        return json_error(f"服务端处理失败：{exc}", 500)
    return str(exc), 500


def serialize_result(result):
    stored = RESULT_STORE.save(result.content, result.filename, result.mime_type)
    payload = {
        "kind": result.kind,
        "filename": stored.filename,
        "size": stored.size,
        "download_url": url_for("download_result", token=stored.token),
    }
    if result.inline_text is not None:
        payload.update(extract_text_payload(result.inline_text, stored.filename, stored.token))
    return payload


@app.get("/")
def index():
    return render_template("index.html")


@app.get("/api/keys")
def list_keys():
    return jsonify({"ok": True, "keys": KEY_STORE.list_all()})


@app.post("/api/keys/import")
def import_key():
    key_text = request.form.get("key_text", "").strip()
    key_file = request.files.get("key_file")

    if key_file and key_file.filename:
        blob: str | bytes = key_file.read()
    elif key_text:
        blob = key_text
    else:
        return json_error("请提供要导入的公钥或私钥。")

    try:
        saved = KEY_STORE.import_blob(blob)
    except KeyStoreError as exc:
        return json_error(str(exc))

    return jsonify(
        {
            "ok": True,
            "message": "凭据已导入本地工具。",
            "saved": saved,
            "keys": KEY_STORE.list_all(),
        }
    )


@app.post("/api/keys/generate")
def generate_key():
    name = request.form.get("name", "").strip()
    email = request.form.get("email", "").strip()
    comment = request.form.get("comment", "").strip()
    passphrase = request.form.get("passphrase", "")

    try:
        key_size = int(request.form.get("key_size", "3072"))
    except ValueError:
        return json_error("凭据位数无效。")

    try:
        key = generate_rsa_key(
            name=name,
            email=email,
            comment=comment,
            passphrase=passphrase,
            key_size=key_size,
        )
        KEY_STORE.save_key(key)
    except (CryptoError, KeyStoreError) as exc:
        return json_error(str(exc))

    fingerprint = KEY_STORE.normalize_fingerprint(str(key.fingerprint))
    return jsonify(
        {
            "ok": True,
            "message": "新的凭据对已生成并保存。",
            "fingerprint": fingerprint,
            "downloads": {
                "public": url_for("download_key", kind="public", fingerprint=fingerprint),
                "private": url_for("download_key", kind="private", fingerprint=fingerprint),
            },
            "keys": KEY_STORE.list_all(),
        }
    )


@app.get("/api/keys/<kind>/<fingerprint>")
def download_key(kind: str, fingerprint: str):
    try:
        asset = KEY_STORE.export_key(kind, fingerprint)
    except KeyStoreError as exc:
        return json_error(str(exc), 404)

    return send_file(
        asset.path,
        as_attachment=True,
        download_name=asset.filename,
        mimetype="text/plain; charset=utf-8",
        max_age=0,
    )


@app.delete("/api/keys/<kind>/<fingerprint>")
def delete_key(kind: str, fingerprint: str):
    try:
        KEY_STORE.delete_key(kind, fingerprint)
    except KeyStoreError as exc:
        return json_error(str(exc), 404)

    return jsonify({"ok": True, "message": "凭据已删除。", "keys": KEY_STORE.list_all()})


@app.post("/api/encrypt")
def encrypt():
    input_type = request.form.get("input_type", "text")
    mode = request.form.get("mode", "symmetric")
    output_format = request.form.get("output_format", "")
    compression = request.form.get("compression", "zlib")

    if output_format not in {"binary", "armor"}:
        output_format = "armor" if input_type == "text" else "binary"

    text_value = request.form.get("text_input", "")
    text_upload = request.files.get("text_input_file")
    upload = request.files.get("file_input")

    kwargs = {
        "input_type": input_type,
        "mode": mode,
        "armor": output_format == "armor",
        "compression_name": compression,
        "passphrase": request.form.get("passphrase", ""),
    }

    if input_type == "text":
        if text_value:
            kwargs["text_value"] = text_value
        elif text_upload and text_upload.filename:
            try:
                kwargs["text_value"] = text_upload.read().decode("utf-8")
            except UnicodeDecodeError:
                return json_error("文本模式只支持 UTF-8 文本内容。")
        else:
            kwargs["text_value"] = ""
    else:
        if upload is None or not upload.filename:
            return json_error("请选择一个要加密的文件。")
        kwargs["file_name"] = upload.filename
        kwargs["file_bytes"] = upload.read()

    if mode == "public":
        fingerprint = request.form.get("public_key_fingerprint", "")
        try:
            kwargs["public_key"] = KEY_STORE.load_key("public", fingerprint)
            kwargs["public_key_fingerprint"] = fingerprint
        except KeyStoreError as exc:
            return json_error(str(exc))

    try:
        result = encrypt_content(**kwargs)
    except (CryptoError, KeyStoreError) as exc:
        return json_error(str(exc))

    return jsonify(
        {
            "ok": True,
            "message": "加密完成。",
            "result": serialize_result(result),
        }
    )


@app.post("/api/decrypt")
def decrypt():
    input_type = request.form.get("input_type", "text")
    mode = request.form.get("mode", "symmetric")
    text_value = request.form.get("ciphertext_text", "")
    text_upload = request.files.get("ciphertext_text_file")
    upload = request.files.get("ciphertext_file")
    passphrase = request.form.get("passphrase", "")

    kwargs = {
        "mode": mode,
        "passphrase": passphrase,
    }

    if input_type == "text":
        if text_value:
            kwargs["encrypted_blob"] = text_value
        elif text_upload and text_upload.filename:
            kwargs["encrypted_blob"] = text_upload.read()
        else:
            kwargs["encrypted_blob"] = ""
    else:
        if upload is None or not upload.filename:
            return json_error("请选择一个要解密的文件。")
        kwargs["encrypted_blob"] = upload.read()

    if mode == "public":
        fingerprint = request.form.get("private_key_fingerprint", "")
        try:
            kwargs["private_key"] = KEY_STORE.load_key("private", fingerprint)
        except KeyStoreError as exc:
            return json_error(str(exc))

    try:
        result = decrypt_content(**kwargs)
    except (CryptoError, KeyStoreError) as exc:
        return json_error(str(exc))

    return jsonify(
        {
            "ok": True,
            "message": "解密完成。",
            "result": serialize_result(result),
        }
    )


@app.get("/api/results/<token>")
def download_result(token: str):
    try:
        asset = RESULT_STORE.get(token)
    except FileNotFoundError as exc:
        return json_error(str(exc), 404)

    return send_file(
        asset.path,
        as_attachment=True,
        download_name=asset.filename,
        mimetype=asset.mime_type,
        max_age=0,
    )


def parse_args():
    parser = argparse.ArgumentParser(description="ShenYin - Local Data Workspace")
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=8765)
    parser.add_argument("--no-browser", action="store_true")
    return parser.parse_args()


def main():
    args = parse_args()
    if not args.no_browser:
        threading.Timer(1.0, lambda: webbrowser.open(f"http://{args.host}:{args.port}")).start()
    app.run(host=args.host, port=args.port, debug=False)


if __name__ == "__main__":
    main()
