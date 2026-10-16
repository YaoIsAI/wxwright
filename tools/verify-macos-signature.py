#!/usr/bin/env python3
"""Offline code-signature verifier for macOS Mach-O binaries (no macOS needed).

Verifies the embedded code signature (LC_CODE_SIGNATURE -> SuperBlob ->
CodeDirectory) of a Mach-O file: parses the CodeDirectory and recomputes every
code-page hash, then compares against the stored slots. Works for linker
ad-hoc signatures and codesign-applied ad-hoc / Developer ID signatures alike.

Why this exists: the v0.11.0 macOS dmg shipped an app bundle with no codesign
seal at all, which Apple Silicon Gatekeeper reports as "damaged". This tool
lets any maintainer verify what a published binary's signature actually is,
directly from the release artifacts (CLI tarball or the app's
Contents/MacOS/<bin> extracted from a mounted dmg), without a Mac.

Usage:
    python tools/verify-macos-signature.py <macho-binary> [more binaries...]

Exit status: 0 = every binary has a valid signature (hashes intact),
             1 = any binary unsigned or hash mismatch.

Interpretation:
    flags CS_LINKER_SIGNED (0x20000)     -> automatic signature from ld;
        a VALID result means the file bytes are untouched since linking, but
        the surrounding .app bundle may still be unsealed (Gatekeeper:
        "damaged" when quarantined).
    flags CS_ADHOC (0x2) without linker  -> codesign -s - sealed it;
        quarantined launches get the recoverable "unnotarized developer"
        dialog instead of "damaged".
    team id present + no adhoc           -> Developer ID; opens cleanly only
        when also notarized+stapled.
"""
import hashlib
import struct
import sys

CS_ADHOC = 0x2
CS_LINKER_SIGNED = 0x20000
CS_HASHTYPES = {1: "sha1", 2: "sha256", 3: "sha512"}
CSMAGIC_EMBEDDED_SIGNATURE = 0xFADE0CC0
CSMAGIC_CODEDIRECTORY = 0xFADE0C02
LC_CODE_SIGNATURE = 0x1D
MH_MAGIC_64 = 0xFEEDFACF
CPU_ARM64 = 0x0100000C
CPU_X86_64 = 0x01000007  # CPU_ARCH_ABI64 | 7


def u32(buf, off):
    """Little-endian u32: Mach-O header and load commands."""
    return struct.unpack("<I", buf[off: off + 4])[0]


def be32(buf, off):
    """Big-endian u32: code-signing blobs (SuperBlob / CodeDirectory)."""
    return struct.unpack(">I", buf[off: off + 4])[0]


def inspect(path):
    with open(path, "rb") as f:
        data = f.read()
    if u32(data, 0) != MH_MAGIC_64:
        print(f"{path}: not a 64-bit little-endian Mach-O (magic {data[:4].hex()}), skipped")
        return None
    cputype = u32(data, 4)
    arch = {CPU_ARM64: "arm64", CPU_X86_64: "x86_64"}.get(cputype, hex(cputype))
    ncmds = u32(data, 16)

    sig = None
    off = 32
    for _ in range(ncmds):
        if off + 8 > len(data):
            break
        cmd, sz = struct.unpack("<II", data[off: off + 8])
        if sz < 8 or off + sz > len(data):
            break
        if cmd == LC_CODE_SIGNATURE:
            sig = struct.unpack("<II", data[off + 8: off + 16])
        off += sz

    if not sig:
        print(f"{path}: [{arch}] UNSIGNED - no LC_CODE_SIGNATURE; "
              f"Apple Silicon kills this on exec")
        return False

    dataoff, datasize = sig
    sb = dataoff
    magic, sb_len, count = struct.unpack(">III", data[sb: sb + 12])
    if magic != CSMAGIC_EMBEDDED_SIGNATURE:
        print(f"{path}: bad SuperBlob magic {magic:#x}")
        return False

    cd = None
    for i in range(count):
        slot, s_off = struct.unpack(">II", data[sb + 12 + 8 * i: sb + 20 + 8 * i])
        if slot == 0 and be32(data, sb + s_off) == CSMAGIC_CODEDIRECTORY:
            cd = sb + s_off
    if cd is None:
        print(f"{path}: SuperBlob has no CodeDirectory")
        return False

    version = be32(data, cd + 8)
    flags = be32(data, cd + 12)
    hash_off = be32(data, cd + 16)
    ident_off = be32(data, cd + 20)
    n_special = be32(data, cd + 24)
    n_code = be32(data, cd + 28)
    code_limit = be32(data, cd + 32)
    hash_size = data[cd + 36]
    hash_type = data[cd + 37]
    page_shift = data[cd + 39]
    ident = data[cd + ident_off: data.index(b"\x00", cd + ident_off)].decode(
        "utf-8", "replace")
    algo = CS_HASHTYPES.get(hash_type, f"type{hash_type}")

    kind = []
    if flags & CS_LINKER_SIGNED:
        kind.append("linker-signed (automatic, bundle may still be unsealed)")
    if flags & CS_ADHOC:
        kind.append("ad-hoc")
    if not kind:
        kind.append("identity-signed")

    bad = 0
    if algo in CS_HASHTYPES.values():
        page = 1 << page_shift
        hashfn = getattr(hashlib, algo)
        for i in range(n_code):
            start = i * page
            end = min(start + page, code_limit)
            digest = hashfn(data[start: end]).digest()
            stored = data[cd + hash_off + i * hash_size:
                          cd + hash_off + (i + 1) * hash_size]
            if digest != stored:
                bad += 1

    ok = bad == 0
    print(f"{path}: [{arch}] ident {ident!r} version {version:#x} flags "
          f"{flags:#x} ({' + '.join(kind)})")
    print(f"    {n_code} code slots ({algo}, limit {code_limit}), "
          f"{n_special} special slots -> "
          f"{'VALID' if ok else f'INVALID ({bad} mismatches)'}")
    return ok


def main():
    if len(sys.argv) < 2:
        print(__doc__)
        return 2
    results = [inspect(p) for p in sys.argv[1:]]
    checked = [r for r in results if r is not None]
    if not checked or not all(checked):
        return 1
    print("all signatures valid")
    return 0


if __name__ == "__main__":
    sys.exit(main())
