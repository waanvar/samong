#!/usr/bin/env python3
"""Rasterise site/brand/social-preview.svg into the PNG that social platforms fetch.

Why this script exists at all
-----------------------------
The card that was shipped before had no reproducible origin: a PNG appeared in
the tree and nothing recorded how. That is fine until the wording changes, at
which point the SVG and the PNG disagree and the PNG is what the world sees.

Why headless Chrome rather than a library
----------------------------------------
The card's headline is live text in Bai Jamjuree, not outlined paths like the
wordmark. A rasteriser that does not load the project's own woff2 files silently
substitutes whatever font it can find, and the card ships in the wrong typeface
while looking plausible. So the SVG is wrapped in an HTML page that declares
@font-face against site/fonts/, and Chrome is asked to wait for
document.fonts.ready before the screenshot is taken.

Why the output filename carries a version
-----------------------------------------
Facebook and X cache what they scrape, per URL, for a long time. Overwriting the
PNG in place leaves everyone who has already shared the link looking at the old
card — and there is no way to purge their cache from here. A new filename is a
new URL, which is the one thing that reliably forces a refetch. Bump CARD and the
og:image in site/index.html together.

Usage:
    python packaging/site/make-social-card.py            # write and verify
    python packaging/site/make-social-card.py --check    # verify only, no write
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
SITE = ROOT / "site"
SVG = SITE / "brand" / "social-preview.svg"

# Bump this and the og:image/twitter:image in site/index.html in the same commit.
CARD = SITE / "brand" / "social-preview-v2.png"

WIDTH, HEIGHT = 1280, 640

CHROME_CANDIDATES = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
]

# The two faces the card actually asks for, and the weights it asks for them in.
FONTS = [
    ("Bai Jamjuree", 700, "bai-jamjuree-latin-700-normal.woff2"),
    ("Bai Jamjuree", 600, "bai-jamjuree-latin-600-normal.woff2"),
    ("Plex Thai", 400, "ibm-plex-sans-thai-latin-400-normal.woff2"),
    ("Plex Mono", 400, "ibm-plex-mono-latin-400-normal.woff2"),
]

PAGE = """<!doctype html>
<meta charset="utf-8">
<title>card</title>
<style>
{faces}
html, body {{ margin: 0; padding: 0; background: #0d1014; }}
svg {{ display: block; }}
</style>
{svg}
<script>
/* Chrome will screenshot a page whose webfonts have not arrived, and the card
   then ships in a fallback face. Holding the title until fonts.ready gives the
   caller something to assert on. */
document.fonts.ready.then(function () {{ document.title = "FONTS-READY"; }});
</script>
"""


def find_chrome() -> str:
    for name in ("chrome", "google-chrome", "chromium"):
        found = shutil.which(name)
        if found:
            return found
    for path in CHROME_CANDIDATES:
        if os.path.exists(path):
            return path
    sys.exit("no Chrome or Chromium found; set one of: " + ", ".join(CHROME_CANDIDATES))


def png_size(data: bytes) -> tuple[int, int]:
    if data[:8] != b"\x89PNG\r\n\x1a\n":
        raise ValueError("not a PNG")
    return struct.unpack(">II", data[16:24])


def build(dest: pathlib.Path) -> None:
    svg = SVG.read_text(encoding="utf-8")
    faces = "\n".join(
        '@font-face {{ font-family: "{}"; font-weight: {}; src: url("{}") format("woff2"); '
        "font-display: block; }}".format(
            family, weight, (SITE / "fonts" / filename).as_uri()
        )
        for family, weight, filename in FONTS
    )
    page = PAGE.format(faces=faces, svg=svg)

    with tempfile.TemporaryDirectory() as tmp:
        tmpdir = pathlib.Path(tmp)
        html = tmpdir / "card.html"
        html.write_text(page, encoding="utf-8")

        chrome = find_chrome()
        common = [
            chrome,
            "--headless=new",
            "--disable-gpu",
            "--no-sandbox",
            "--no-first-run",
            "--hide-scrollbars",
            "--force-device-scale-factor=1",
            "--user-data-dir=" + str(tmpdir / "profile"),
            "--window-size={},{}".format(WIDTH, HEIGHT),
            "--virtual-time-budget=8000",
        ]

        # Assert the fonts loaded before trusting the pixels. Without this the
        # card can ship in a substitute face and look merely slightly wrong.
        dom = subprocess.run(
            common + ["--dump-dom", html.as_uri()],
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        ).stdout
        if "FONTS-READY" not in dom:
            sys.exit("the webfonts never became ready; the card would ship in a fallback face")

        subprocess.run(
            common + ["--screenshot=" + str(dest), html.as_uri()],
            capture_output=True,
            check=False,
        )

    if not dest.exists():
        sys.exit("Chrome produced no screenshot at " + str(dest))
    w, h = png_size(dest.read_bytes())
    if (w, h) != (WIDTH, HEIGHT):
        sys.exit("card is {}x{}, expected {}x{}".format(w, h, WIDTH, HEIGHT))
    # Facebook rejects images over 8MB and X over 5MB; a card this simple should
    # be well under 300KB, so anything larger means something went wrong.
    size = dest.stat().st_size
    if size > 5_000_000:
        sys.exit("card is {} bytes, too large for X's 5MB limit".format(size))
    print("card: {}x{}, {} bytes -> {}".format(w, h, size, dest.name))


def check_meta() -> None:
    """Every image the page promises a crawler must exist, at the size claimed.

    This is the failure the versioned filename introduces: bump og:image and
    forget to commit the PNG, and the page keeps serving 200 while the card
    silently becomes a bare link again — indistinguishable, from here, from the
    platform caching. Nothing else in CI looks at these tags.
    """
    import re

    html = (SITE / "index.html").read_text(encoding="utf-8")
    refs = re.findall(
        r'<meta\s+(?:property|name)="((?:og|twitter):image(?::secure_url)?)"\s+content="([^"]+)"',
        html,
    )
    if not refs:
        sys.exit("no og:image or twitter:image in site/index.html at all")

    declared = {}
    for prop in ("og:image:width", "og:image:height"):
        m = re.search(r'<meta\s+property="%s"\s+content="(\d+)"' % prop, html)
        if m:
            declared[prop] = int(m.group(1))

    prefix = "https://samong.dev/"
    for tag, url in refs:
        if not url.startswith(prefix):
            sys.exit("{} is not an absolute samong.dev URL: {}".format(tag, url))
        path = SITE / url[len(prefix):]
        if not path.exists():
            sys.exit("{} points at {}, which is not in site/".format(tag, url[len(prefix):]))
        w, h = png_size(path.read_bytes())
        if declared and (w, h) != (declared.get("og:image:width"), declared.get("og:image:height")):
            sys.exit("{} is {}x{} but the page declares {}x{}".format(
                path.name, w, h, declared.get("og:image:width"), declared.get("og:image:height")))
        print("{:26} -> {} ({}x{}, {} bytes)".format(tag, path.name, w, h, path.stat().st_size))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true",
                    help="rebuild into a temporary file and compare, without writing")
    ap.add_argument("--meta-only", action="store_true",
                    help="only verify site/index.html's image tags resolve; no Chrome needed")
    args = ap.parse_args()

    if args.meta_only:
        check_meta()
        return

    if not args.check:
        build(CARD)
        check_meta()
        return

    check_meta()

    if not CARD.exists():
        sys.exit("{} does not exist; run without --check to create it".format(CARD.name))
    with tempfile.TemporaryDirectory() as tmp:
        fresh = pathlib.Path(tmp) / "fresh.png"
        build(fresh)
        # Compared by size rather than byte equality: Chrome's PNG encoder is not
        # deterministic across versions, so an exact match would fail for reasons
        # that have nothing to do with the card being out of date.
        old = CARD.stat().st_size
        new = fresh.stat().st_size
        drift = abs(old - new) / max(old, 1)
        print("committed {} bytes, fresh {} bytes ({:.1%} apart)".format(old, new, drift))
        if drift > 0.10:
            sys.exit("the committed card is more than 10% off a fresh render of the SVG; "
                     "rerun without --check and commit the result")


if __name__ == "__main__":
    main()
