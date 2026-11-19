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
            if platform.startswith("linux"):
                p.linux_desktop_files(stage)
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

    def test_icon_files_have_complete_native_formats(self):
        data = (p.ICONS / "minecraft.ico").read_bytes()
        reserved, kind, count = struct.unpack_from("<HHH", data)
        self.assertEqual((reserved, kind), (0, 1))
        sizes = set()
        for index in range(count):
            width, height, _, _, _, _, size, offset = struct.unpack_from("<BBBBHHII", data, 6 + index * 16)
            self.assertEqual(width, height)
            sizes.add(width or 256)
            self.assertGreater(size, 0)
            self.assertLessEqual(offset + size, len(data))
        self.assertEqual(sizes, {16, 24, 32, 48, 64, 128, 256})
        self.assertEqual(len((p.ICONS / "minecraft.rgba").read_bytes()), 32 * 32 * 4)
        data = (p.ICONS / "minecraft.icns").read_bytes()
        self.assertEqual(data[:4], b"icns")
        self.assertEqual(struct.unpack_from(">I", data, 4)[0], len(data))
        data = (p.ICONS / "minecraft.png").read_bytes()
        self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
        self.assertEqual(struct.unpack_from(">II", data, 16), (256, 256))

    def test_mac_bundle_declares_and_contains_icon(self):
        import plistlib
        with tempfile.TemporaryDirectory() as temporary:
            stage, executable = self.fixture(Path(temporary), "macos-aarch64")
            info_path = stage / p.APP / "Contents/Info.plist"
            info = plistlib.loads(info_path.read_bytes())
            self.assertEqual(info["CFBundleIconFile"], "minecraft.icns")
            icon = stage / p.APP / "Contents/Resources/minecraft.icns"
            self.assertEqual(icon.read_bytes(), (p.ICONS / "minecraft.icns").read_bytes())
            icon.unlink()
            archive = Path(temporary) / "missing-icon.zip"
            p.make_archive(stage, archive, executable)
            with self.assertRaisesRegex(ValueError, "icon missing"):
                p.verify_archive(archive, "macos-aarch64", "0.1.0")

    def test_linux_icon_and_desktop_entry_are_required_and_verified(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stage, executable = self.fixture(root, "linux-x86_64")
            desktop = stage / p.DESKTOP
            entry = desktop.read_text()
            self.assertIn("Icon=dev.oxide.launcher", entry)
            self.assertIn("Exec=oxide-launcher", entry)
            desktop.write_text(entry.replace("Exec=oxide-launcher", "Exec=wrong-binary"))
            archive = root / "bad-desktop.tar.gz"
            p.make_archive(stage, archive, executable)
            with self.assertRaisesRegex(ValueError, "desktop entry or icon"):
                p.verify_archive(archive, "linux-x86_64", "0.1.0")

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
