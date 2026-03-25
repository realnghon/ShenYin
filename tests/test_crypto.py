from __future__ import annotations

import unittest

from pgpbox.crypto import decrypt_content, encrypt_content


class CryptoTests(unittest.TestCase):
    def test_symmetric_text_roundtrip(self):
        encrypted = encrypt_content(
            input_type="text",
            armor=True,
            compression_name="zlib",
            passphrase="secret",
            text_value="hello armored world",
        )

        self.assertEqual(encrypted.kind, "text")
        self.assertTrue(all(32 <= ord(c) <= 126 or c in "\r\n" for c in encrypted.inline_text))

        decrypted = decrypt_content(
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
        )

        self.assertEqual(decrypted.kind, "text")
        self.assertEqual(decrypted.inline_text, "hello armored world")

    def test_symmetric_file_roundtrip(self):
        original = bytes(range(64))
        encrypted = encrypt_content(
            input_type="file",
            armor=False,
            compression_name="zip",
            passphrase="secret",
            file_name="archive.zip",
            file_bytes=original,
        )

        decrypted = decrypt_content(
            encrypted_blob=encrypted.content,
            passphrase="secret",
        )

        self.assertEqual(decrypted.kind, "download")
        self.assertEqual(decrypted.filename, "archive.zip")
        self.assertEqual(decrypted.content, original)

    def test_symmetric_file_to_text_roundtrip(self):
        encrypted = encrypt_content(
            input_type="file",
            armor=True,
            compression_name="zlib",
            passphrase="secret",
            file_name="notes.bin",
            file_bytes=b"\x00\x01\x02hello",
        )

        self.assertEqual(encrypted.kind, "text")

        decrypted = decrypt_content(
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
        )

        self.assertEqual(decrypted.kind, "download")
        self.assertEqual(decrypted.filename, "notes.bin")
        self.assertEqual(decrypted.content, b"\x00\x01\x02hello")

    def test_wrong_passphrase_raises(self):
        encrypted = encrypt_content(
            input_type="text",
            armor=True,
            compression_name="zlib",
            passphrase="correct",
            text_value="secret data",
        )

        with self.assertRaises(Exception):
            decrypt_content(
                encrypted_blob=encrypted.inline_text,
                passphrase="wrong",
            )

    def test_bz2_compression_roundtrip(self):
        encrypted = encrypt_content(
            input_type="text",
            armor=True,
            compression_name="bz2",
            passphrase="secret",
            text_value="hello bz2",
        )

        decrypted = decrypt_content(
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
        )

        self.assertEqual(decrypted.inline_text, "hello bz2")

    def test_no_compression_roundtrip(self):
        encrypted = encrypt_content(
            input_type="text",
            armor=True,
            compression_name="none",
            passphrase="secret",
            text_value="hello none",
        )

        decrypted = decrypt_content(
            encrypted_blob=encrypted.inline_text,
            passphrase="secret",
        )

        self.assertEqual(decrypted.inline_text, "hello none")


if __name__ == "__main__":
    unittest.main()
