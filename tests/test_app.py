from __future__ import annotations

from io import BytesIO
import unittest

from app import app


class AppTests(unittest.TestCase):
    def setUp(self):
        self.client = app.test_client()

    def test_index_available(self):
        response = self.client.get("/")
        self.assertEqual(response.status_code, 200)

    def test_generate_encrypt_decrypt_flow(self):
        generate = self.client.post(
            "/api/keys/generate",
            data={
                "name": "Han",
                "email": "han@example.com",
                "comment": "flow",
                "passphrase": "pw",
                "key_size": "2048",
            },
        )
        self.assertEqual(generate.status_code, 200)
        payload = generate.get_json()
        self.assertTrue(payload["ok"])
        fingerprint = payload["fingerprint"]

        encrypt = self.client.post(
            "/api/encrypt",
            data={
                "input_type": "text",
                "mode": "public",
                "output_format": "armor",
                "compression": "zlib",
                "text_input": "hello flow",
                "public_key_fingerprint": fingerprint,
            },
        )
        self.assertEqual(encrypt.status_code, 200)
        encrypted_text = encrypt.get_json()["result"]["text"]

        decrypt = self.client.post(
            "/api/decrypt",
            data={
                "input_type": "text",
                "mode": "public",
                "ciphertext_text": encrypted_text,
                "passphrase": "pw",
                "private_key_fingerprint": fingerprint,
            },
        )
        self.assertEqual(decrypt.status_code, 200)
        decrypted = decrypt.get_json()
        self.assertEqual(decrypted["result"]["text"], "hello flow")

    def test_text_encrypt_no_pgp_markers(self):
        response = self.client.post(
            "/api/encrypt",
            data={
                "input_type": "text",
                "mode": "symmetric",
                "compression": "zlib",
                "passphrase": "pw",
                "text_input": "text transport",
            },
        )
        self.assertEqual(response.status_code, 200)
        payload = response.get_json()
        self.assertTrue(payload["ok"])
        text = payload["result"]["text"]
        self.assertNotIn("BEGIN PGP MESSAGE", text)
        self.assertTrue(all(32 <= ord(c) <= 126 or c in "\r\n" for c in text))

    def test_file_encrypt_to_text_then_upload_decrypt(self):
        encrypt = self.client.post(
            "/api/encrypt",
            data={
                "input_type": "file",
                "mode": "symmetric",
                "output_format": "armor",
                "compression": "zlib",
                "passphrase": "pw",
                "file_input": (BytesIO(b"hello file"), "demo.txt"),
            },
            content_type="multipart/form-data",
        )
        self.assertEqual(encrypt.status_code, 200)
        encoded_text = encrypt.get_json()["result"]["text"]
        self.assertNotIn("BEGIN PGP MESSAGE", encoded_text)

        decrypt = self.client.post(
            "/api/decrypt",
            data={
                "input_type": "file",
                "mode": "symmetric",
                "passphrase": "pw",
                "ciphertext_file": (BytesIO(encoded_text.encode("utf-8")), "demo.txt.txt"),
            },
            content_type="multipart/form-data",
        )
        self.assertEqual(decrypt.status_code, 200)
        payload = decrypt.get_json()
        self.assertEqual(payload["result"]["filename"], "demo.txt")

    def test_large_file_encrypt_download_large_text_then_text_decrypt(self):
        original = bytes((index * 37 + 17) % 256 for index in range(300_000))

        encrypt = self.client.post(
            "/api/encrypt",
            data={
                "input_type": "file",
                "mode": "symmetric",
                "output_format": "armor",
                "compression": "none",
                "passphrase": "pw",
                "file_input": (BytesIO(original), "large.bin"),
            },
            content_type="multipart/form-data",
        )
        self.assertEqual(encrypt.status_code, 200)
        payload = encrypt.get_json()["result"]
        self.assertTrue(payload["text_too_large"])
        self.assertFalse(payload["text_available"])
        self.assertNotIn("text", payload)

        download = self.client.get(payload["download_url"])
        self.assertEqual(download.status_code, 200)
        encoded_text = download.data.decode("utf-8")
        download.close()
        self.assertNotIn("BEGIN PGP MESSAGE", encoded_text)

        decrypt = self.client.post(
            "/api/decrypt",
            data={
                "input_type": "text",
                "mode": "symmetric",
                "passphrase": "pw",
                "ciphertext_text": encoded_text,
            },
        )
        self.assertEqual(decrypt.status_code, 200)
        decrypted = decrypt.get_json()["result"]
        self.assertEqual(decrypted["filename"], "large.bin")

    def test_large_file_encrypt_download_then_buffered_text_decrypt(self):
        original = bytes((index * 53 + 11) % 256 for index in range(600_000))

        encrypt = self.client.post(
            "/api/encrypt",
            data={
                "input_type": "file",
                "mode": "symmetric",
                "output_format": "armor",
                "compression": "none",
                "passphrase": "pw",
                "file_input": (BytesIO(original), "buffered.bin"),
            },
            content_type="multipart/form-data",
        )
        self.assertEqual(encrypt.status_code, 200)
        payload = encrypt.get_json()["result"]
        self.assertTrue(payload["text_too_large"])

        download = self.client.get(payload["download_url"])
        self.assertEqual(download.status_code, 200)
        encoded_bytes = download.data
        download.close()

        decrypt = self.client.post(
            "/api/decrypt",
            data={
                "input_type": "text",
                "mode": "symmetric",
                "passphrase": "pw",
                "ciphertext_text_file": (BytesIO(encoded_bytes), "pasted-message.txt"),
            },
            content_type="multipart/form-data",
        )
        self.assertEqual(decrypt.status_code, 200)
        decrypted = decrypt.get_json()["result"]
        self.assertEqual(decrypted["filename"], "buffered.bin")

    def test_large_text_post_is_not_rejected_by_form_memory_limit(self):
        response = self.client.post(
            "/api/decrypt",
            data={
                "input_type": "text",
                "mode": "symmetric",
                "passphrase": "pw",
                "ciphertext_text": "A" * 1_200_000,
            },
        )
        self.assertNotEqual(response.status_code, 413)


if __name__ == "__main__":
    unittest.main()
