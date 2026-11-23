#!/usr/bin/env python3
"""Build a native Oxide Launcher package without copying game/user data or publishing."""
import argparse
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import stat
import struct
import subprocess
import tarfile
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parent.parent
TARGETS = {
    "x86_64-pc-windows-msvc": "windows-x86_64",
    "x86_64-unknown-linux-gnu": "linux-x86_64",
    "x86_64-apple-darwin": "macos-x86_64",
    "aarch64-apple-darwin": "macos-aarch64",
}
APP = "Oxide Launcher.app"
ICONS = ROOT / "assets/icons"
DESKTOP = "dev.oxide.launcher.desktop"


def command(*args, cwd=ROOT):
    result = subprocess.run(
        args, cwd=cwd, check=True, capture_output=True, text=True, timeout=30
    )
    return result.stdout.strip()


def sha256(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def archive_name(platform, version):
    suffix = ".tar.gz" if platform.startswith("linux") else ".zip"
    return f"Oxide-Launcher-v{version}-{platform}{suffix}"


def verify_native_header(data, platform):
    if platform.startswith("windows"):
        if data[:2] != b"MZ" or len(data) < 64:
            raise ValueError("Expected a Windows executable")
        offset = struct.unpack_from("<I", data, 0x3c)[0]
        if (
            len(data) < offset + 6
            or data[offset:offset + 4] != b"PE\x00\x00"
            or struct.unpack_from("<H", data, offset + 4)[0] != 0x8664
        ):
            raise ValueError("Expected a Windows x86_64 executable")
    elif platform.startswith("linux"):
        if len(data) < 20 or data[:6] != b"\x7fELF\x02\x01" or struct.unpack_from("<H", data, 18)[0] != 62:
            raise ValueError("Expected a Linux x86_64 executable")
    else:
        cpu = 0x0100000c if platform.endswith("aarch64") else 0x01000007
        if len(data) < 8 or data[:4] != b"\xcf\xfa\xed\xfe" or struct.unpack_from("<I", data, 4)[0] != cpu:
            raise ValueError("Expected the matching native Mac executable")


def mac_bundle(binary, stage, version):
    bundle = stage / APP
    executable = bundle / "Contents/MacOS/oxide-launcher"
    executable.parent.mkdir(parents=True)
    shutil.copy2(binary, executable)
    executable.chmod(0o755)
    info = {
        "CFBundleInfoDictionaryVersion": "6.0",
        "CFBundleIdentifier": "dev.oxide.launcher",
        "CFBundleName": "Oxide Launcher",
        "CFBundleDisplayName": "Oxide Launcher",
        "CFBundleExecutable": "oxide-launcher",
        "CFBundlePackageType": "APPL",
        "CFBundleIconFile": "minecraft.icns",
        "CFBundleShortVersionString": version,
        "CFBundleVersion": version,
        "LSMinimumSystemVersion": "12.0",
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
    }
    resources = bundle / "Contents/Resources"
    resources.mkdir()
    shutil.copy2(ICONS / "minecraft.icns", resources / "minecraft.icns")
    (bundle / "Contents/Info.plist").write_bytes(plistlib.dumps(info))
    return bundle, executable


def linux_desktop_files(stage):
    shutil.copy2(ICONS / "minecraft.png", stage / "oxide-launcher.png")
    shutil.copy2(ICONS / DESKTOP, stage / DESKTOP)


def make_archive(stage, destination, executable):
    # Stage contains only the native launcher, platform icon/desktop files, and README.
    files = sorted(path for path in stage.rglob("*") if path.is_file())
    if destination.name.endswith(".zip"):
        with zipfile.ZipFile(destination, "x", compression=zipfile.ZIP_DEFLATED) as archive:
            for path in files:
                info = zipfile.ZipInfo(path.relative_to(stage).as_posix())
                info.create_system = 3
                info.external_attr = (stat.S_IFREG | (0o755 if path == executable else 0o644)) << 16
                archive.writestr(info, path.read_bytes(), compress_type=zipfile.ZIP_DEFLATED)
    else:
        with tarfile.open(destination, "x:gz") as archive:
            for path in files:
                info = archive.gettarinfo(path, arcname=path.relative_to(stage).as_posix())
                info.mode = 0o755 if path == executable else 0o644
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                with path.open("rb") as stream:
                    archive.addfile(info, stream)


def verify_archive(path, platform, version):
    archive = zipfile.ZipFile(path) if path.name.endswith(".zip") else tarfile.open(path)
    with archive:
        zip_format = isinstance(archive, zipfile.ZipFile)
        names = archive.namelist() if zip_format else archive.getnames()
        def read(name):
            return archive.read(name) if zip_format else archive.extractfile(name).read()
        def mode(name):
            return archive.getinfo(name).external_attr >> 16 if zip_format else archive.getmember(name).mode
        if "README.txt" not in names or len(names) != len(set(names)):
            raise ValueError("Invalid launcher archive inventory")
        if platform.startswith("macos"):
            executable = APP + "/Contents/MacOS/oxide-launcher"
            icon = APP + "/Contents/Resources/minecraft.icns"
            allowed = {"README.txt", APP + "/Contents/Info.plist", executable, icon}
            if not allowed.issubset(names):
                raise ValueError("Mac launcher app or icon missing")
            extra = set(names) - allowed
            if any(not name.startswith(APP + "/Contents/_CodeSignature/") for name in extra):
                raise ValueError("Unexpected file in Mac launcher archive")
            info = plistlib.loads(read(APP + "/Contents/Info.plist"))
            if info["CFBundleShortVersionString"] != version or info["CFBundleExecutable"] != "oxide-launcher":
                raise ValueError("Incorrect Mac app metadata")
            if info.get("CFBundleIconFile") != "minecraft.icns" or read(icon) != (ICONS / "minecraft.icns").read_bytes():
                raise ValueError("Incorrect Mac app icon")
        else:
            executable = "oxide-launcher.exe" if platform.startswith("windows") else "oxide-launcher"
            expected = {executable, "README.txt"}
            if platform.startswith("linux"):
                expected.update({"oxide-launcher.png", DESKTOP})
            if set(names) != expected:
                raise ValueError("Unexpected file in launcher archive")
            if platform.startswith("linux"):
                if read("oxide-launcher.png") != (ICONS / "minecraft.png").read_bytes() or read(DESKTOP) != (ICONS / DESKTOP).read_bytes():
                    raise ValueError("Incorrect Linux desktop entry or icon")
        if executable not in names or mode(executable) & 0o111 != 0o111:
            raise ValueError("Launcher executable or permissions missing")
        if not zip_format and any(not entry.isfile() for entry in archive.getmembers()):
            raise ValueError("Archive links/special files are not supported")
        data = read(executable)
        verify_native_header(data, platform)
        return hashlib.sha256(data).hexdigest()


def readme(version, platform):
    return f"""Oxide Launcher v{version} | {platform}

Extract the entire archive. Windows: run oxide-launcher.exe.
Linux: run ./oxide-launcher in a graphical X11/Wayland session.
For a Linux application-menu entry with its Minecraft icon, run these commands
from the extracted folder (ensure ~/.local/bin is on your session PATH):
  install -Dm755 oxide-launcher "$HOME/.local/bin/oxide-launcher"
  install -Dm644 dev.oxide.launcher.desktop "$HOME/.local/share/applications/dev.oxide.launcher.desktop"
  install -Dm644 oxide-launcher.png "$HOME/.local/share/icons/hicolor/256x256/apps/dev.oxide.launcher.png"
macOS: open Oxide Launcher.app. These builds are ad-hoc signed, not notarized.

Enter an offline player name and choose Install & Play. The launcher downloads
the appropriate game build from https://github.com/OldSchoolMinecraft/Oxide/releases/latest.
The game is downloaded separately; its packages include textures and audio.
Use Profile Editor for launch settings. Oxide News supports Markdown release notes.
Game updates do not update the launcher executable itself.

Settings, installed games, saves and logs stay in per-user application directories.
Replacing the launcher does not remove them. Local executable mode is also available.

Requirements: a compatible graphics device or the software-rendering fallback.
Linux: glibc 2.35+, X11/Wayland, xkbcommon and installed system fonts.
macOS: 12 or newer. Mac/Linux desktop GUI validation remains outstanding.
Windows: 64-bit Windows with compatible system graphics drivers.

Game downloads: https://github.com/OldSchoolMinecraft/Oxide
Launcher source: https://github.com/OldSchoolMinecraft/Oxide-Launcher
The Minecraft grass-block icon is Mojang artwork.
Not affiliated with Mojang or Microsoft.
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=TARGETS)
    parser.add_argument("--output-dir", type=Path)
    parser.add_argument("--skip-build", action="store_true")
    args = parser.parse_args()
    host = next(line[6:] for line in command("rustc", "-vV").splitlines() if line.startswith("host: "))
    target = args.target or host
    if target not in TARGETS or target != host:
        parser.error("Use a native runner matching the requested target")
    platform = TARGETS[target]
    metadata = json.loads(command("cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"))
    version = next(p["version"] for p in metadata["packages"] if p["name"] == "oxide-launcher")
    destination = (args.output_dir or ROOT / ".local/builds" / f"v{version}" / platform).resolve()
    if destination.exists():
        parser.error("Refusing to overwrite an existing build directory")
    if not args.skip_build:
        subprocess.run(["cargo", "build", "--release", "--locked", "--target", target], cwd=ROOT, check=True)
    name = "oxide-launcher.exe" if platform.startswith("windows") else "oxide-launcher"
    binary = Path(metadata["target_directory"]) / target / "release" / name
    expected_version = "oxide-launcher " + version
    if command(str(binary), "--version") != expected_version:
        raise ValueError("Native executable version mismatch")
    destination.parent.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="oxide-launcher-", dir=destination.parent) as temporary:
        work = Path(temporary)
        stage, public = work / "stage", work / "public"
        stage.mkdir()
        public.mkdir()
        if platform.startswith("macos"):
            bundle, executable = mac_bundle(binary, stage, version)
            subprocess.run(["codesign", "--force", "--deep", "--sign", "-", str(bundle)], check=True)
            subprocess.run(["codesign", "--verify", "--strict", "--deep", str(bundle)], check=True)
        else:
            executable = stage / name
            shutil.copy2(binary, executable)
            if platform.startswith("linux"):
                linux_desktop_files(stage)
        (stage / "README.txt").write_text(readme(version, platform), encoding="utf-8", newline="\n")
        archive = public / archive_name(platform, version)
        make_archive(stage, archive, executable)
        if verify_archive(archive, platform, version) != sha256(executable):
            raise ValueError("Packaged executable hash mismatch")
        if command(str(executable), "--version", cwd=stage) != expected_version:
            raise ValueError("Packaged executable failed headless startup")
        (public / "SHA256SUMS.txt").write_text(f"{sha256(archive)}  {archive.name}\n", encoding="ascii", newline="\n")
        provenance = {"version": version, "target": target, "sourceCommit": command("git", "rev-parse", "HEAD"),
                      "sourceStatus": command("git", "status", "--porcelain"), "rustc": command("rustc", "-vV"),
                      "executableSha256": sha256(executable), "archiveSha256": sha256(archive)}
        (work / "provenance.private.json").write_text(json.dumps(provenance, indent=2), encoding="utf-8")
        shutil.move(str(work), destination)
    print("Prepared launcher package: " + str(destination / "public"))


if __name__ == "__main__":
    main()
