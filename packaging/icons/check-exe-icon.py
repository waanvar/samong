#!/usr/bin/env python3
"""Read the icon back out of a Windows executable.

`build.rs` warns rather than fails when a resource compiler is missing, so that a
contributor without the Windows SDK is not blocked by a cosmetic resource. The
cost of that leniency is that a build can succeed while producing an .exe with no
icon, and nothing in the build output says so.

This is the other half. It walks the PE resource directory and reports the icon
sizes and version strings actually embedded — so a release can assert on what is
in the file rather than on the absence of a warning.

Usage:
    python packaging/icons/check-exe-icon.py <exe> [<exe> ...]
    python packaging/icons/check-exe-icon.py --require 16,32,48,256 <exe>
"""

from __future__ import annotations

import argparse
import struct
import sys

RT_ICON = 3
RT_GROUP_ICON = 14
RT_VERSION = 16


class Pe:
    def __init__(self, data: bytes):
        self.d = data
        if data[:2] != b"MZ":
            raise ValueError("not a PE file (no MZ)")
        pe = struct.unpack_from("<I", data, 0x3C)[0]
        if data[pe:pe + 4] != b"PE\0\0":
            raise ValueError("not a PE file (no PE signature)")
        coff = pe + 4
        n_sections, = struct.unpack_from("<H", data, coff + 2)
        opt_size, = struct.unpack_from("<H", data, coff + 16)
        opt = coff + 20
        magic, = struct.unpack_from("<H", data, opt)
        # The data directory sits after the optional header's fixed part, whose
        # length differs between PE32 and PE32+.
        dd = opt + (96 if magic == 0x10B else 112)
        self.res_rva, self.res_size = struct.unpack_from("<II", data, dd + 8 * 2)

        self.sections = []
        st = opt + opt_size
        for i in range(n_sections):
            off = st + 40 * i
            name = data[off:off + 8].rstrip(b"\0").decode("ascii", "replace")
            vsize, vaddr, rawsize, rawptr = struct.unpack_from("<IIII", data, off + 8)
            self.sections.append((name, vaddr, vsize, rawptr, rawsize))

    def offset(self, rva: int) -> int:
        for _, vaddr, vsize, rawptr, rawsize in self.sections:
            if vaddr <= rva < vaddr + max(vsize, rawsize):
                return rawptr + (rva - vaddr)
        raise ValueError("RVA 0x%x is in no section" % rva)

    def walk(self, base: int, off: int, level: int = 0, path=()):
        """Yield (path_of_ids, data_rva, size) for every leaf in the resource tree."""
        named, ids = struct.unpack_from("<HH", self.d, off + 12)
        for i in range(named + ids):
            e = off + 16 + 8 * i
            name_or_id, child = struct.unpack_from("<II", self.d, e)
            if child & 0x80000000:
                yield from self.walk(base, base + (child & 0x7FFFFFFF), level + 1,
                                     path + (name_or_id,))
            else:
                rva, size = struct.unpack_from("<II", self.d, base + child)
                yield path + (name_or_id,), rva, size

    def resources(self):
        if not self.res_rva:
            return []
        base = self.offset(self.res_rva)
        return list(self.walk(base, base))


def icon_sizes(pe: Pe) -> list[int]:
    """Sizes declared by the RT_GROUP_ICON directory — what Windows actually picks
    from. A width byte of 0 means 256, which is how the format encodes it."""
    out: list[int] = []
    for path, rva, size in pe.resources():
        if not path or path[0] != RT_GROUP_ICON:
            continue
        blob = pe.d[pe.offset(rva):pe.offset(rva) + size]
        count, = struct.unpack_from("<H", blob, 4)
        for i in range(count):
            w = blob[6 + 14 * i]
            out.append(w or 256)
    return sorted(set(out))


def version_strings(pe: Pe) -> list[str]:
    """Crude but sufficient: the version resource stores UTF-16 key/value pairs, and
    we only need to prove ours are present."""
    found = []
    for path, rva, size in pe.resources():
        if not path or path[0] != RT_VERSION:
            continue
        blob = pe.d[pe.offset(rva):pe.offset(rva) + size]
        text = blob.decode("utf-16-le", "replace")
        for key in ("ProductName", "FileDescription", "LegalCopyright"):
            if key in text:
                found.append(key)
    return found


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("exes", nargs="+")
    ap.add_argument("--require", default="",
                    help="comma-separated icon sizes that must all be present")
    args = ap.parse_args()

    required = [int(s) for s in args.require.split(",") if s.strip()]
    failed = False

    for path in args.exes:
        try:
            pe = Pe(open(path, "rb").read())
        except Exception as exc:
            print("::error::%s: %s" % (path, exc))
            failed = True
            continue

        sizes = icon_sizes(pe)
        n_icons = sum(1 for p, _, _ in pe.resources() if p and p[0] == RT_ICON)
        vers = version_strings(pe)
        print("%-24s icon sizes %-28s RT_ICON x%-3d version fields %s"
              % (path.split("\\")[-1].split("/")[-1], sizes or "NONE", n_icons,
                 ",".join(vers) or "NONE"))

        if not sizes:
            print("::error::%s has no icon embedded" % path)
            failed = True
        missing = [s for s in required if s not in sizes]
        if missing:
            print("::error::%s is missing icon sizes %s" % (path, missing))
            failed = True

    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
