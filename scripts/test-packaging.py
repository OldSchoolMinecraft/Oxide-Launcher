#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import struct
import tempfile
import unittest


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


p = module("package-build")
c = module("collect-builds")


class PackagingTests(unittest.TestCase):
    def fixture(self, root, platform):
        stage = root / (platform + "-stage")
        stage.mkdir()
        binary = root / (platform + "-binary")
        data = bytearray(128)
        if platform.startswith("windows"):
            data[:2] = b"MZ"
            struct.pack_into("<I", data, 0x3c, 64)
            data[64:68] = b"PE\x00\x00"
            struct.pack_into("<H", data, 68, 0x8664)
        elif platform.startswith("linux"):
            data[:6] = b"\x7fELF\x02\x01"
            struct.pack_into("<H", data, 18, 62)
        else:
            data[:4] = b"\xcf\xfa\xed\xfe"
            struct.pack_into("<I", data, 4, 0x0100000c if platform.endswith("aarch64") else 0x01000007)
        binary.write_bytes(data)
        if platform.startswith("macos"):
            _, executable = p.mac_bundle(binary, stage, "0.1.0")
        else:
            executable = stage / ("oxide-launcher.exe" if platform.startswith("windows") else "oxide-launcher")
            executable.write_bytes(data)
        (stage / "README.txt").write_text("Launcher instructions")
        return stage, executable

    def test_all_platform_archives_and_wrong_architecture(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for platform in p.TARGETS.values():
                stage, executable = self.fixture(root, platform)
                extension = ".tar.gz" if platform.startswith("linux") else ".zip"
                archive = root / (platform + extension)
                p.make_archive(stage, archive, executable)
                self.assertEqual(p.verify_archive(archive, platform, "0.1.0"), p.sha256(executable))
                with self.assertRaises(FileExistsError):
                    p.make_archive(stage, archive, executable)
                if platform == "macos-aarch64":
                    with self.assertRaises(ValueError):
                        p.verify_archive(archive, "macos-x86_64", "0.1.0")
                    with self.assertRaises(ValueError):
                        p.verify_archive(archive, platform, "9.0.0")

    def test_collect_requires_complete_verified_platform_set(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            downloads = root / "downloads"
            for platform in p.TARGETS.values():
                stage, executable = self.fixture(root, platform)
                folder = downloads / ("launcher-" + platform)
                folder.mkdir(parents=True)
                extension = ".tar.gz" if platform.startswith("linux") else ".zip"
                archive = folder / f"Oxide-Launcher-v0.1.0-{platform}{extension}"
                p.make_archive(stage, archive, executable)
                (folder / "SHA256SUMS.txt").write_text(f"{p.sha256(archive)}  {archive.name}\n")
            result = c.collect(downloads, root / "public", "0.1.0")
            self.assertEqual(len(result), 4)
            self.assertEqual(len(list((root / "public").iterdir())), 5)
            with self.assertRaises(ValueError):
                c.collect(downloads, root / "public", "0.1.0")
            result[0][0].write_bytes(b"corrupted")
            with self.assertRaises(ValueError):
                c.collect(downloads, root / "bad-output", "0.1.0")
            self.assertFalse((root / "bad-output").exists())


if __name__ == "__main__":
    unittest.main()
