#!/usr/bin/env python3
"""Derive every icon the packaging needs from assets/icon/samong-icon.svg.

Produces, all from the one master:

    assets/icon/samong-<n>.png   16..1024, for Linux's hicolor theme
    assets/icon/samong.ico       Windows, multi-image
    assets/icon/samong.icns      macOS app bundle

Why headless Chrome for the rasterising
---------------------------------------
The master uses gradients and a stroked round-rect. Chrome is the renderer whose
output we can check by eye, and it is already the project's rasteriser for the
share card. Rendering each size natively rather than downscaling one big PNG
matters at 16 and 32, where hinting the geometry to the pixel grid is the whole
difference between a legible icon and grey mush.

Why the .icns is written by hand
--------------------------------
Pillow reads ICNS but only writes it on macOS, where it shells out to Apple's
tooling. The container is simple enough to emit directly — a magic word, a total
length, then one length-prefixed chunk per size with a PNG inside — so the file
can be produced on the machine this project is actually developed on.
"""

from __future__ import annotations

import argparse
import os
import pathlib
import shutil
import struct
import subprocess
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
MASTER = ROOT / "assets" / "icon" / "samong-icon.svg"
# Distinct artwork for the sizes where the full mark cannot be drawn — see the
# comment at the top of that file. Sizes at or below SMALL_UPTO come from it.
MASTER_SMALL = ROOT / "assets" / "icon" / "samong-icon-small.svg"
SMALL_UPTO = 24
OUT = ROOT / "assets" / "icon"

# Linux themes and general use. 24 is included because Windows asks for it in the
# .ico and it is cheap to have.
PNG_SIZES = [16, 24, 32, 48, 64, 128, 256, 512, 1024]

# Windows caps .ico entries at 256; anything larger is not addressable.
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]

# OSType codes macOS looks for. The ic* codes are the retina-era PNG ones; icp4/5
# cover the small sizes that Finder and the menu bar still ask for.
ICNS_CHUNKS = [
    ("icp4", 16),
    ("icp5", 32),
    ("icp6", 64),
    ("ic07", 128),
    ("ic08", 256),
    ("ic09", 512),
    ("ic11", 32),    # 16pt @2x
    ("ic12", 64),    # 32pt @2x
    ("ic13", 256),   # 128pt @2x
    ("ic14", 512),   # 256pt @2x
    ("ic10", 1024),  # 512pt @2x
]

CHROME_CANDIDATES = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
]


def find_chrome() -> str:
    for name in ("chrome", "google-chrome", "chromium"):
        found = shutil.which(name)
        if found:
            return found
    for path in CHROME_CANDIDATES:
        if os.path.exists(path):
            return path
    sys.exit("no Chrome or Chromium found")


def render(chrome: str, size: int, dest: pathlib.Path, tmpdir: pathlib.Path,
           source: pathlib.Path | None = None) -> None:
    """Render an SVG at exactly `size`, as its own page."""
    svg = (source or MASTER).read_text(encoding="utf-8")
    # The page is the icon and nothing else: no margin, no scrollbars, transparent
    # outside the tile so rounded corners stay rounded.
    html = (
        "<!doctype html><meta charset='utf-8'>"
        "<style>html,body{{margin:0;padding:0;background:transparent}}"
        "svg{{display:block;width:{s}px;height:{s}px}}</style>{svg}"
    ).format(s=size, svg=svg)
    page = tmpdir / ("icon-%d.html" % size)
    page.write_text(html, encoding="utf-8")

    subprocess.run(
        [
            chrome, "--headless=new", "--disable-gpu", "--no-sandbox", "--no-first-run",
            "--hide-scrollbars", "--force-device-scale-factor=1",
            "--default-background-color=00000000",   # keep the corners transparent
            "--user-data-dir=" + str(tmpdir / ("p%d" % size)),
            "--window-size=%d,%d" % (size, size),
            "--virtual-time-budget=4000",
            "--screenshot=" + str(dest),
            page.as_uri(),
        ],
        capture_output=True,
        check=False,
    )
    if not dest.exists():
        sys.exit("Chrome produced no PNG at size %d" % size)


# The tile fills its frame except for the rounded corners, so every size should
# land near this. The first run produced a 128px PNG at 23.8% with no lime node at
# all — Chrome had screenshotted it part-way through painting. Nothing about the
# file was malformed: right dimensions, valid PNG, and it would have gone into
# both the .ico and the .icns to be wrong at one size only.
MIN_OPAQUE = 0.85
MIN_LIME = 0.02


def coverage(path: pathlib.Path) -> tuple[float, float]:
    from PIL import Image

    im = Image.open(path).convert("RGBA")
    px = list(im.getdata())
    n = len(px) or 1
    opaque = sum(1 for _, _, _, a in px if a > 8)
    lime = sum(1 for r, g, b, a in px if a > 8 and g > 150 and g - b > 50)
    return opaque / n, lime / n


"""Why one render at 1024 and Pillow for the rest, instead of rendering each size.

Rendering each size natively was the first approach, on the theory that hinting
the geometry to the pixel grid at 16 and 32 beats a downscale. It produced a
128px icon truncated to its top 32 rows — deterministically, three times over.

Sweeping the sizes found the rule: 16 to 80 render fully, 160 and up render fully,
and every size in between comes out clipped to exactly `size - 96` rows tall.
96 itself renders one row. Whatever the mechanism in headless Chrome's window
sizing, a generator that depends on which side of that band a number falls is a
generator that breaks the next time someone adds a size. So: one render at 1024,
which is comfortably in the known-good range, and Pillow's LANCZOS for everything
below it. One code path, no quirk to remember.

The master was already drawn with heavy enough strokes to survive being made
small, which is what makes the downscale acceptable at 16px.
"""


def render_master(chrome: str, tmpdir: pathlib.Path, dest: pathlib.Path) -> None:
    from PIL import Image

    render(chrome, 1024, dest, tmpdir)
    if Image.open(dest).size != (1024, 1024):
        sys.exit("the master rendered as %s, not 1024x1024" % (Image.open(dest).size,))
    opaque, lime = coverage(dest)
    if opaque < MIN_OPAQUE or lime < MIN_LIME:
        sys.exit("the master render is incomplete: %.1f%% opaque, %.2f%% lime"
                 % (100 * opaque, 100 * lime))
    print("  master 1024  opaque %.1f%%  lime %.2f%%" % (100 * opaque, 100 * lime))


def build_icns(pngs: dict[int, pathlib.Path], dest: pathlib.Path) -> None:
    """Write an ICNS container directly.

    Layout: b'icns', total length as big-endian u32, then for each image a 4-byte
    OSType, a big-endian u32 length *including* the 8-byte header, then the PNG.
    """
    chunks = []
    for code, size in ICNS_CHUNKS:
        data = pngs[size].read_bytes()
        chunks.append(code.encode("ascii") + struct.pack(">I", len(data) + 8) + data)
    body = b"".join(chunks)
    dest.write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true",
                    help="verify the committed icons exist and are well-formed; no rendering")
    args = ap.parse_args()

    if args.check:
        check()
        return

    from PIL import Image

    OUT.mkdir(parents=True, exist_ok=True)
    chrome = find_chrome()
    pngs: dict[int, pathlib.Path] = {}

    with tempfile.TemporaryDirectory() as tmp:
        tmpdir = pathlib.Path(tmp)
        master = OUT / "samong-1024.png"
        render_master(chrome, tmpdir, master)
        pngs[1024] = master

        # The small artwork is rendered at 512, which is inside the range headless
        # Chrome draws completely, and downscaled the same way as the full one.
        small_master = tmpdir / "small-512.png"
        render(chrome, 512, small_master, tmpdir, source=MASTER_SMALL)
        opaque, lime = coverage(small_master)
        if opaque < MIN_OPAQUE or lime < MIN_LIME:
            sys.exit("the small-size master render is incomplete: %.1f%% opaque, %.2f%% lime"
                     % (100 * opaque, 100 * lime))
        print("  small  512  opaque %.1f%%  lime %.2f%%" % (100 * opaque, 100 * lime))

        wanted = sorted(set(PNG_SIZES) | set(ICO_SIZES) | {s for _, s in ICNS_CHUNKS})
        src = Image.open(master).convert("RGBA")
        src_small = Image.open(small_master).convert("RGBA")
        for size in wanted:
            if size == 1024:
                continue
            dest = OUT / ("samong-%d.png" % size)
            base = src_small if size <= SMALL_UPTO else src
            base.resize((size, size), Image.LANCZOS).save(dest, format="PNG", optimize=True)
            opaque, lime = coverage(dest)
            if opaque < MIN_OPAQUE or lime < MIN_LIME:
                sys.exit("size %d came out wrong after downscaling: %.1f%% opaque, %.2f%% lime"
                         % (size, 100 * opaque, 100 * lime))
            pngs[size] = dest
            print("  %-4d %7d bytes  opaque %4.1f%%  lime %4.2f%%"
                  % (size, dest.stat().st_size, 100 * opaque, 100 * lime))

        # Pillow writes a genuine multi-image ICO when given explicit sizes.
        Image.open(pngs[1024]).save(
            OUT / "samong.ico", format="ICO",
            sizes=[(s, s) for s in ICO_SIZES],
        )
        build_icns(pngs, OUT / "samong.icns")

    print("ico  : %d bytes" % (OUT / "samong.ico").stat().st_size)
    print("icns : %d bytes" % (OUT / "samong.icns").stat().st_size)
    check()


def check() -> None:
    """What the CI job asserts: the files exist, and each really contains what its
    format promises. A truncated or single-image .ico installs fine and looks wrong
    only at one specific size, which is exactly the sort of thing nobody notices."""
    from PIL import Image

    ico = OUT / "samong.ico"
    icns = OUT / "samong.icns"
    problems = []

    for size in PNG_SIZES:
        p = OUT / ("samong-%d.png" % size)
        if not p.exists():
            problems.append("missing %s" % p.name)
            continue
        if Image.open(p).size != (size, size):
            problems.append("%s is %s" % (p.name, Image.open(p).size))
            continue
        # Content, not just shape. The first version of this check verified the
        # dimensions and the container and passed a 128px icon that was three
        # quarters empty — a message claiming more than the check performed.
        opaque, lime = coverage(p)
        if opaque < MIN_OPAQUE:
            problems.append("%s is only %.1f%% opaque — an incomplete render"
                            % (p.name, 100 * opaque))
        if lime < MIN_LIME:
            problems.append("%s has no lit node (%.2f%% lime) — an incomplete render"
                            % (p.name, 100 * lime))

    if not ico.exists():
        problems.append("missing samong.ico")
    else:
        im = Image.open(ico)
        have = sorted(im.ico.sizes()) if hasattr(im, "ico") else []
        missing = [s for s in ICO_SIZES if (s, s) not in have]
        if missing:
            problems.append("samong.ico lacks sizes %s (has %s)" % (missing, have))
        else:
            # Each frame separately: the container can carry the right number of
            # images and one of them still be blank.
            for s in ICO_SIZES:
                frame = im.ico.getimage((s, s)).convert("RGBA")
                px = list(frame.getdata())
                op = sum(1 for _, _, _, a in px if a > 8) / (len(px) or 1)
                if op < MIN_OPAQUE:
                    problems.append("samong.ico frame %dx%d is only %.1f%% opaque"
                                    % (s, s, 100 * op))

    if not icns.exists():
        problems.append("missing samong.icns")
    else:
        raw = icns.read_bytes()
        if raw[:4] != b"icns":
            problems.append("samong.icns has no icns magic")
        elif struct.unpack(">I", raw[4:8])[0] != len(raw):
            problems.append("samong.icns length header %d != file size %d"
                            % (struct.unpack(">I", raw[4:8])[0], len(raw)))
        else:
            # Walk the chunks so a truncated tail cannot pass.
            pos, found = 8, []
            import io
            while pos < len(raw):
                code = raw[pos:pos + 4].decode("ascii", "replace")
                ln = struct.unpack(">I", raw[pos + 4:pos + 8])[0]
                if ln < 8 or pos + ln > len(raw):
                    problems.append("samong.icns chunk %r has bad length %d" % (code, ln))
                    break
                # The payload has to be a PNG of the size the OSType promises, or
                # macOS silently falls back to a generic icon.
                payload = raw[pos + 8:pos + ln]
                want = dict(ICNS_CHUNKS).get(code)
                try:
                    got = Image.open(io.BytesIO(payload)).size
                except Exception as exc:
                    problems.append("samong.icns chunk %r is not a readable image (%s)"
                                    % (code, exc))
                else:
                    if want and got != (want, want):
                        problems.append("samong.icns chunk %r holds %s, expected %dx%d"
                                        % (code, got, want, want))
                found.append(code)
                pos += ln
            expected = [c for c, _ in ICNS_CHUNKS]
            if found != expected:
                problems.append("samong.icns chunks %s != %s" % (found, expected))

    if problems:
        for p in problems:
            print("::error::" + p)
        sys.exit(1)
    print("icons ok: %d PNGs, .ico with %d sizes, .icns with %d chunks"
          % (len(PNG_SIZES), len(ICO_SIZES), len(ICNS_CHUNKS)))


if __name__ == "__main__":
    main()
