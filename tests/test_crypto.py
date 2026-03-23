from __future__ import annotations

import unittest

from pgpbox.crypto import decrypt_content, encrypt_content, generate_rsa_key


class CryptoTests(unittest.TestCase):
    def test_symmetric_text_roundtrip(self):
        encrypted = encrypt_content(
            input_type="text",
            mode="symmetric",
            armor=True,
            compression_name="zlib",
            passphrase="secret",
            text_value="hello armored world",
        )

        self.assertEqual(encrypted.kind, "text")
        self.assertIn("BEGIN PGP MESSAGE", encrypted.inline_text)

        decrypted = decrypt_content(
            mode="symmetric",
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
        )

        self.assertEqual(decrypted.kind, "text")
        self.assertEqual(decrypted.inline_text, "hello armored world")

    def test_symmetric_file_roundtrip(self):
        original = bytes(range(64))
        encrypted = encrypt_content(
            input_type="file",
            mode="symmetric",
            armor=False,
            compression_name="zip",
            passphrase="secret",
            file_name="archive.zip",
            file_bytes=original,
        )

        decrypted = decrypt_content(
            mode="symmetric",
            encrypted_blob=encrypted.content,
            passphrase="secret",
        )

        self.assertEqual(decrypted.kind, "download")
        self.assertEqual(decrypted.filename, "archive.zip")
        self.assertEqual(decrypted.content, original)

    def test_public_key_roundtrip(self):
        key = generate_rsa_key(
            name="Han",
            email="han@example.com",
            comment="",
            passphrase="secret",
            key_size=2048,
        )

        encrypted = encrypt_content(
            input_type="text",
            mode="public",
            armor=True,
            compression_name="zlib",
            text_value="hello public key",
            public_key=key.pubkey,
        )

        decrypted = decrypt_content(
            mode="public",
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
            private_key=key,
        )

        self.assertEqual(decrypted.inline_text, "hello public key")

    def test_symmetric_file_to_armored_text_roundtrip(self):
        encrypted = encrypt_content(
            input_type="file",
            mode="symmetric",
            armor=True,
            compression_name="zlib",
            passphrase="secret",
            file_name="notes.bin",
            file_bytes=b"\x00\x01\x02hello",
        )

        self.assertEqual(encrypted.kind, "text")
        self.assertIn("BEGIN PGP MESSAGE", encrypted.inline_text)

        decrypted = decrypt_content(
            mode="symmetric",
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
        )

        self.assertEqual(decrypted.kind, "download")
        self.assertEqual(decrypted.filename, "notes.bin")
        self.assertEqual(decrypted.content, b"\x00\x01\x02hello")


if __name__ == "__main__":
    unittest.main()
