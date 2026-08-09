"""Point the PKGBUILD at the newest release.

Three things move together — `pkgver`, `_archive` and `sha256sums` — and a mismatch
in any one of them fails differently: a wrong `_archive` breaks `package()` after the
download succeeds, a wrong checksum fails before it, and a stale `pkgver` installs
an old build under a new version number.

`.SRCINFO` cannot be produced here: it comes from `makepkg --printsrcinfo`, which
needs Arch. CI generates it in a container and fails if the committed copy differs,
printing what it should be.

Usage:
  generate.py            rewrite for the newest release
  generate.py --check    fail if the PKGBUILD is not what would be written
  generate.py --version 0.4.0
"""

import json
import os
import re
import sys
import urllib.request
from pathlib import Path

REPO = "waanvar/samong"
PKGBUILD = Path(__file__).parent / "PKGBUILD"
TIMEOUT = 60
SHA256 = re.compile(r"^[a-f0-9]{64}$")


def get(url: str) -> bytes:
    """Fetch a URL, authenticating API calls when a token is in the environment.

    Unauthenticated GitHub API requests are limited to **60 per hour per IP**, and
    a shared Actions runner burns that between jobs belonging to other people
    entirely — so an unauthenticated call fails with `403: rate limit exceeded` at
    unpredictable times and looks like a bug in whatever ran it. With
    `GITHUB_TOKEN` the limit is 5,000/hour.

    The header goes only to `api.github.com`. Release asset URLs redirect to
    `objects.githubusercontent.com`, which rejects an Authorization header it did
    not expect — so sending it everywhere would trade an intermittent 403 for a
    reliable 400.
    """
    headers = {"User-Agent": "aur-samong-generate"}
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token and url.startswith("https://api.github.com/"):
        headers["Authorization"] = "Bearer " + token
    request = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(request, timeout=TIMEOUT) as response:  # noqa: S310
        return response.read()


def latest_tag() -> str:
    return json.loads(get(f"https://api.github.com/repos/{REPO}/releases/latest"))["tag_name"]


def rewrite(text: str, version: str, digest: str) -> str:
    archive = f"samong-v{version}-x86_64-linux"
    replacements = [
        (r"^_archive=.*$", f"_archive={archive}"),
        (r"^pkgver=.*$", f"pkgver={version}"),
        # pkgrel returns to 1 on a version change: it counts packaging fixes for a
        # given upstream version, so carrying it forward would claim this build is
        # the second attempt at something it has never packaged.
        (r"^pkgrel=.*$", "pkgrel=1"),
        (r"^sha256sums=\(.*\)$", f"sha256sums=('{digest}')"),
    ]
    for pattern, replacement in replacements:
        text, count = re.subn(pattern, replacement, text, count=1, flags=re.M)
        if count != 1:
            raise SystemExit(f"could not find {pattern!r} in the PKGBUILD")
    return text


def main() -> int:
    args = sys.argv[1:]
    if "--version" in args:
        version = args[args.index("--version") + 1].lstrip("v")
    else:
        version = latest_tag().lstrip("v")

    archive = f"samong-v{version}-x86_64-linux"
    digest = get(
        f"https://github.com/{REPO}/releases/download/v{version}/{archive}.tar.gz.sha256"
    ).decode("utf-8").split()[0]
    if not SHA256.match(digest):
        raise SystemExit(f"no sha256 published for {archive}.tar.gz")

    current = PKGBUILD.read_text(encoding="utf-8")
    wanted = rewrite(current, version, digest)

    if "--check" in args:
        if current.replace("\r\n", "\n") != wanted.replace("\r\n", "\n"):
            print("PKGBUILD is not what the generator produces for " + version, file=sys.stderr)
            print("run: python packaging/aur/generate.py", file=sys.stderr)
            return 1
        print(f"PKGBUILD matches the generator for {version}")
        return 0

    if current == wanted:
        print(f"PKGBUILD is already at {version}")
        return 0

    PKGBUILD.write_text(wanted, encoding="utf-8")
    written = PKGBUILD.read_text(encoding="utf-8")
    for needle in (f"pkgver={version}", f"_archive={archive}", f"sha256sums=('{digest}')"):
        if needle not in written:
            raise SystemExit(f"{needle} was not written")
    print(f"PKGBUILD rewritten for {version} ({digest[:16]}…)")
    print("regenerate .SRCINFO on Arch: makepkg --printsrcinfo > .SRCINFO")
    return 0


if __name__ == "__main__":
    sys.exit(main())
