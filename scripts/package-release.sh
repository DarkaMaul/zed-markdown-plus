#!/usr/bin/env bash
set -euo pipefail

check_version() {
  local tag="$1"
  python3 - "$tag" <<'PY'
import sys
import tomllib
from pathlib import Path

tag = sys.argv[1]
if not tag.startswith("v"):
    raise SystemExit(f"Expected a v-prefixed release tag, got {tag!r}")
expected = tag[1:]
versions = {
    "root Cargo.toml": tomllib.loads(Path("Cargo.toml").read_text())["package"]["version"],
    "server/Cargo.toml": tomllib.loads(Path("server/Cargo.toml").read_text())["package"]["version"],
    "extension.toml": tomllib.loads(Path("extension.toml").read_text())["version"],
}
if any(version != expected for version in versions.values()):
    raise SystemExit(f"Tag {tag!r} must match all package versions {versions}")
print(f"Release tag {tag} matches all package versions")
PY
}

case "${1:-}" in
  check-version)
    [[ $# -eq 2 ]] || { echo "usage: $0 check-version TAG" >&2; exit 2; }
    check_version "$2"
    ;;
  package)
    [[ $# -eq 3 ]] || { echo "usage: $0 package TAG PLATFORM" >&2; exit 2; }
    tag="$2"
    platform="$3"
    check_version "$tag"
    case "$platform" in
      linux-x86_64|macos-aarch64) ;;
      *) echo "unsupported release platform: $platform" >&2; exit 2 ;;
    esac
    case "$(uname -s):$(uname -m)" in
      Linux:x86_64) host_platform=linux-x86_64 ;;
      Darwin:arm64) host_platform=macos-aarch64 ;;
      *) echo "unsupported build host: $(uname -s) $(uname -m)" >&2; exit 2 ;;
    esac
    if [[ "$platform" != "$host_platform" ]]; then
      echo "requested platform $platform does not match build host $host_platform" >&2
      exit 2
    fi

    cargo build --release --locked -p markdown-folds-server
    python3 tests/protocol.py

    mkdir -p dist
    package_dir="$(mktemp -d)"
    trap 'rm -rf "$package_dir"' EXIT
    archive="markdown-folds-server-${tag}-${platform}.tar.gz"
    cp target/release/markdown-folds-server "$package_dir/"
    cp LICENSE NOTICE README.md "$package_dir/"
    COPYFILE_DISABLE=1 tar -czf "dist/$archive" -C "$package_dir" \
      markdown-folds-server LICENSE NOTICE README.md
    python3 - "$archive" <<'PY'
import hashlib
import pathlib
import sys

archive = pathlib.Path("dist") / sys.argv[1]
digest = hashlib.sha256(archive.read_bytes()).hexdigest()
archive.with_suffix(archive.suffix + ".sha256").write_text(
    f"{digest}  {archive.name}\n"
)
PY
    ;;
  *)
    echo "usage: $0 {check-version TAG|package TAG PLATFORM}" >&2
    exit 2
    ;;
esac
