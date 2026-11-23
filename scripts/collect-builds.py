#!/usr/bin/env python3
"""Verify and collect all launcher platform archives; never publish them."""
import argparse
import importlib.util
from pathlib import Path
import shutil
import tomllib

spec = importlib.util.spec_from_file_location(
    "package_build", Path(__file__).with_name("package-build.py")
)
package_build = importlib.util.module_from_spec(spec)
spec.loader.exec_module(package_build)


def collect(downloads, destination, version):
    if destination.exists():
        raise ValueError("Refusing to replace existing collected packages")
    archives = []
    for platform in package_build.TARGETS.values():
        name = package_build.archive_name(platform, version)
        root = downloads / ("launcher-" + platform)
        path = root / name
        digest = package_build.sha256(path)
        if (root / "SHA256SUMS.txt").read_text(encoding="ascii").strip() != f"{digest}  {name}":
            raise ValueError("Checksum mismatch: " + name)
        package_build.verify_archive(path, platform, version)
        archives.append((path, digest))
    destination.mkdir(parents=True)
    for path, _ in archives:
        shutil.copy2(path, destination / path.name)
    (destination / "SHA256SUMS.txt").write_text(
        "".join(f"{digest}  {path.name}\n" for path, digest in archives), encoding="ascii", newline="\n")
    return archives


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("downloads", type=Path)
    parser.add_argument("destination", type=Path)
    args = parser.parse_args()
    manifest = (package_build.ROOT / "Cargo.toml").read_text(encoding="utf-8")
    version = tomllib.loads(manifest)["package"]["version"]
    for path, digest in collect(args.downloads, args.destination, version):
        print(f"{digest}  {path.name}")
