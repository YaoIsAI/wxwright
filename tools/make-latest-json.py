#!/usr/bin/env python3
"""Compose the Tauri v2 updater manifest (latest.json) from release artifacts.

Tauri's updater downloads one artifact per platform, verifies it against the
minisign signature embedded in the manifest, then installs it:

  Windows  NSIS setup.exe              (passive install, /R restarts the app)
  macOS    wxwright_<arch>.app.tar.gz  (renamed per architecture in CI)
  Linux    AppImage                    (in-place replacement)

If no `.sig` files are present, updater signing has not been configured yet:
the script prints a warning, writes nothing and exits 0 so an unsigned
release still ships. Once one signature exists, every expected platform must
carry one, otherwise the release would be silently half-updatable.

Usage:
  python tools/make-latest-json.py --dir artifacts --tag v0.18.0 \
      --repo YaoIsAI/wxwright --notes "See the release notes" \
      --output artifacts/latest.json
"""

import argparse
import base64
import datetime
import json
import sys
from pathlib import Path
from urllib.parse import quote

EXPECTED = ["windows-x86_64", "darwin-aarch64", "darwin-x86_64", "linux-x86_64"]


def fail(message):
    print(f"::error::{message}", file=sys.stderr)
    sys.exit(1)


def minisign_key_id(blob, label):
    """The 8-byte key id from a minisign public key or signature text.

    Both formats carry `algorithm(2) || key_id(8)` inside the first base64
    line. tauri.conf.json stores the whole public-key file base64-encoded, so
    accept either the encoded blob or its decoded text.
    """
    text = blob.strip()
    if "\n" not in text:
        try:
            text = base64.b64decode(text).decode("utf-8")
        except (ValueError, UnicodeDecodeError):
            fail(f"{label} is neither minisign text nor base64 of it")
    for line in text.splitlines():
        line = line.strip()
        if not line or line.startswith(("untrusted comment:", "trusted comment:")):
            continue
        try:
            raw = base64.b64decode(line)
        except ValueError:
            continue
        if len(raw) >= 10:
            return raw[2:10]
    fail(f"no minisign key id found in {label}")


def pick(root, pattern):
    """Return the single non-signature artifact matching `pattern`."""
    found = [p for p in root.glob(pattern) if not p.name.endswith(".sig")]
    if len(found) > 1:
        fail(f"{pattern} matched more than one artifact: {[p.name for p in found]}")
    return found[0] if found else None


def entry(artifact, repo, tag, expected_key_id):
    """Build one manifest entry, or None when the signature is absent."""
    sig = artifact.with_name(artifact.name + ".sig")
    if not sig.is_file() or not sig.read_text(encoding="utf-8").strip():
        return None
    signature = sig.read_text(encoding="utf-8").strip()
    if expected_key_id is not None:
        signed_by = minisign_key_id(signature, sig.name)
        if signed_by != expected_key_id:
            fail(
                f"{sig.name} was signed by key {signed_by.hex()} but tauri.conf.json "
                f"pins {expected_key_id.hex()} - clients would reject every update"
            )
    url = f"https://github.com/{repo}/releases/download/{tag}/{quote(artifact.name)}"
    return {"signature": signature, "url": url}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--dir", required=True, help="directory with downloaded release artifacts")
    parser.add_argument("--tag", required=True, help="release tag, e.g. v0.18.0")
    parser.add_argument("--repo", required=True, help="GitHub repository, owner/name")
    parser.add_argument("--notes", default="", help="short release notes embedded in the feed")
    parser.add_argument("--output", required=True, help="path of the latest.json to write")
    parser.add_argument(
        "--pubkey",
        help="minisign public key as pinned in tauri.conf.json; defaults to "
        "reading gui/src-tauri/tauri.conf.json when it exists",
    )
    args = parser.parse_args()

    root = Path(args.dir)
    if not root.is_dir():
        fail(f"artifact directory not found: {root}")
    if not args.tag.startswith("v"):
        fail(f"tag must start with 'v': {args.tag}")

    pubkey = args.pubkey
    if not pubkey:
        conf = Path("gui/src-tauri/tauri.conf.json")
        if conf.is_file():
            try:
                pubkey = json.loads(conf.read_text(encoding="utf-8"))["plugins"]["updater"]["pubkey"]
            except (KeyError, ValueError):
                fail(f"{conf} exists but carries no plugins.updater.pubkey")
    expected_key_id = minisign_key_id(pubkey, "updater pubkey") if pubkey else None
    if expected_key_id is None:
        print(
            "::warning title=Updater key unchecked::No pubkey available - "
            "signatures are published without cross-checking the signing key."
        )

    any_sig = any(root.rglob("*.sig"))
    if not any_sig:
        print(
            "::warning title=Updater feed skipped::No updater signatures found. "
            "Set TAURI_SIGNING_PRIVATE_KEY / TAURI_SIGNING_PRIVATE_KEY_PASSWORD "
            "in the repository secrets to publish online updates."
        )
        return

    artifacts = {
        "windows-x86_64": pick(root, "*-setup.exe"),
        "darwin-aarch64": pick(root, "*_aarch64.app.tar.gz"),
        "darwin-x86_64": pick(root, "*_x86_64.app.tar.gz"),
        "linux-x86_64": pick(root, "*.AppImage"),
    }
    missing = [name for name in EXPECTED if artifacts[name] is None]
    if missing:
        fail(f"missing updater artifacts for: {missing}")

    platforms = {}
    for name in EXPECTED:
        item = entry(artifacts[name], args.repo, args.tag, expected_key_id)
        if item is None:
            fail(f"{artifacts[name].name} has no .sig file - was the build signed?")
        platforms[name] = item

    # The plugin probes `windows-x86_64-nsis` first for an NSIS-installed app,
    # then falls back to `windows-x86_64`. Both point at the same installer so
    # installed and portable runs resolve the update.
    platforms["windows-x86_64-nsis"] = dict(platforms["windows-x86_64"])

    # Linux is installer-aware: a deb-installed app asks for
    # `linux-x86_64-deb` before the generic key, and installing the AppImage
    # into /usr/bin would be wrong. Publish the deb entry when the release
    # carries one. tauri-bundler does not sign .deb, so release.yml signs it
    # with `cargo tauri signer sign` before the artifacts are uploaded.
    platforms["linux-x86_64-appimage"] = dict(platforms["linux-x86_64"])
    deb = pick(root, "*.deb")
    if deb is not None:
        deb_entry = entry(deb, args.repo, args.tag, expected_key_id)
        if deb_entry is None:
            fail(f"{deb.name} has no .sig file - was the build signed?")
        platforms["linux-x86_64-deb"] = deb_entry

    manifest = {
        "version": args.tag.lstrip("v"),
        "notes": args.notes,
        "pub_date": datetime.datetime.now(datetime.timezone.utc)
        .replace(microsecond=0)
        .isoformat()
        .replace("+00:00", "Z"),
        "platforms": platforms,
    }
    out = Path(args.output)
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"latest.json -> {out} ({len(platforms)} platform entries)")
    for key, item in platforms.items():
        print(f"  {key}: {item['url']}")


if __name__ == "__main__":
    main()
