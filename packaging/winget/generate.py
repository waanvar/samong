"""Generate the three winget manifests for a released version.

winget wants a directory of three YAML files whose `PackageVersion`, installer URL,
SHA-256 and four `RelativeFilePath` entries all agree. Hand-editing them means six
places to keep in step, and the failure is silent until Microsoft's validation
pipeline rejects the pull request days later.

So they are generated, and CI asserts that regenerating produces exactly the
committed files. The committed copies exist to be read and validated; the generator
is the source of truth.

The digest comes from the `.sha256` published beside the archive rather than from
downloading 30 MB.

Usage:
  generate.py            regenerate manifests/ for the newest release
  generate.py --check    fail if the committed files are not what would be generated
  generate.py --version 0.4.0 [--out DIR]
"""

import json
import os
import re
import sys
import urllib.request
from pathlib import Path

REPO = "waanvar/samong"
IDENTIFIER = "Waanvar.Samong"
# The manifests live in their own directory because `winget validate --manifest`
# parses every file it is pointed at — with the generator alongside them it failed
# with "The manifest does not contain a valid root. File: generate.py".
HERE = Path(__file__).parent / "manifests"
TIMEOUT = 60
SHA256 = re.compile(r"^[a-f0-9]{64}$")
# winget's own examples use upper case; validation accepts either, but matching the
# convention keeps a submitted pull request from looking foreign.
BINARIES = ["samong", "samong-server", "samong-mcp", "samong-app"]


def get(url: str) -> bytes:
    """Fetch a URL, authenticating API calls when a token is in the environment.

    Unauthenticated GitHub API requests are limited to **60 per hour per IP**, and
    a shared Actions runner burns that between jobs belonging to other people
    entirely — so an unauthenticated call fails with `403: rate limit exceeded` at
    unpredictable times and looks like a bug in whatever ran it. With
    `GITHUB_TOKEN` the limit is 5,000/hour. The Homebrew tap's scheduled bump had
    failed five runs in a row on exactly this before it was noticed.

    The header goes only to `api.github.com`. Release asset URLs redirect to
    `objects.githubusercontent.com`, which rejects an Authorization header it did
    not expect — so sending it everywhere would trade an intermittent 403 for a
    reliable 400.
    """
    headers = {"User-Agent": "winget-samong-generate"}
    token = os.environ.get("GH_TOKEN") or os.environ.get("GITHUB_TOKEN")
    if token and url.startswith("https://api.github.com/"):
        headers["Authorization"] = "Bearer " + token
    request = urllib.request.Request(url, headers=headers)
    with urllib.request.urlopen(request, timeout=TIMEOUT) as response:  # noqa: S310
        return response.read()


def latest() -> tuple[str, str]:
    data = json.loads(get(f"https://api.github.com/repos/{REPO}/releases/latest"))
    return data["tag_name"], (data.get("published_at") or "")[:10]


def published_at(tag: str) -> str:
    """The publish date of one specific tag, or "" if it cannot be determined.

    Empty rather than an exception: the caller falls back to the date already in
    the committed manifest, which keeps the generator usable without network
    access. A wrong date is worth failing over; being offline is not.
    """
    try:
        data = json.loads(get(f"https://api.github.com/repos/{REPO}/releases/tags/{tag}"))
    except Exception as exc:  # noqa: BLE001 - any failure means "no date available"
        print(f"could not read the release date for {tag} ({exc}); keeping the committed one",
              file=sys.stderr)
        return ""
    return (data.get("published_at") or "")[:10]


def render(version: str, digest: str, release_date: str) -> dict[str, str]:
    stem = f"samong-v{version}-x86_64-windows"
    url = f"https://github.com/{REPO}/releases/download/v{version}/{stem}.zip"

    nested = "\n".join(
        f"  - RelativeFilePath: {stem}\\{name}.exe\n    PortableCommandAlias: {name}"
        for name in BINARIES
    )

    return {
        f"{IDENTIFIER}.yaml": f"""# yaml-language-server: $schema=https://aka.ms/winget-manifest.version.1.6.0.schema.json
PackageIdentifier: {IDENTIFIER}
PackageVersion: {version}
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.6.0
""",
        f"{IDENTIFIER}.locale.en-US.yaml": f"""# yaml-language-server: $schema=https://aka.ms/winget-manifest.defaultLocale.1.6.0.schema.json
PackageIdentifier: {IDENTIFIER}
PackageVersion: {version}
PackageLocale: en-US
Publisher: waanvar
PublisherUrl: https://github.com/waanvar
PublisherSupportUrl: https://github.com/{REPO}/issues
PackageName: Samong
PackageUrl: https://samong.dev
License: Apache-2.0
LicenseUrl: https://github.com/{REPO}/blob/main/LICENSE
Copyright: Copyright (c) waanvar
ShortDescription: Local-first, Obsidian-compatible knowledge base with full-text search
Description: |-
  Samong indexes a folder of Markdown notes and makes them findable. Full-text
  search handles languages written without spaces between words, such as Thai;
  [[wikilinks]] form a graph you can browse; and an MCP server exposes the same
  notes to AI agents.

  Notes are plain files in a folder you already have. Nothing is uploaded
  anywhere, there is no account, and the local server binds 127.0.0.1 only.
Moniker: samong
Tags:
- knowledge-base
- local-first
- markdown
- mcp
- note-taking
- notes
- obsidian
- rust
- search
ReleaseNotesUrl: https://github.com/{REPO}/blob/main/CHANGELOG.md
ManifestType: defaultLocale
ManifestVersion: 1.6.0
""",
        f"{IDENTIFIER}.installer.yaml": f"""# yaml-language-server: $schema=https://aka.ms/winget-manifest.installer.1.6.0.schema.json
PackageIdentifier: {IDENTIFIER}
PackageVersion: {version}
MinimumOSVersion: 10.0.0.0
InstallerType: zip
NestedInstallerType: portable
InstallModes:
- silent
- silentWithProgress
UpgradeBehavior: uninstallPrevious
ReleaseDate: {release_date}
Installers:
- Architecture: x64
  InstallerUrl: {url}
  InstallerSha256: {digest.upper()}
  NestedInstallerFiles:
{nested}
ManifestType: installer
ManifestVersion: 1.6.0
""",
    }


def main() -> int:
    args = sys.argv[1:]
    check = "--check" in args
    out = HERE
    if "--out" in args:
        out = Path(args[args.index("--out") + 1])

    if "--version" in args:
        version = args[args.index("--version") + 1].lstrip("v")
        # Ask for *this* tag's publish date rather than falling through to the
        # committed file's. Keeping the old value was how `--version 0.4.0`
        # produced a manifest carrying v0.3.9's ReleaseDate: every line of the
        # diff looked right, and the wrong date would have been submitted to
        # Microsoft. Falls back to whatever is committed only if the lookup fails,
        # so an offline run still works.
        release_date = published_at("v" + version)
    else:
        tag, release_date = latest()
        version = tag.lstrip("v")

    stem = f"samong-v{version}-x86_64-windows"
    digest = get(
        f"https://github.com/{REPO}/releases/download/v{version}/{stem}.zip.sha256"
    ).decode("utf-8").split()[0]
    if not SHA256.match(digest):
        raise SystemExit(f"no sha256 published for {stem}.zip")

    if not release_date:
        # Only the installer manifest carries a date; keep whatever is committed
        # rather than inventing one, so --version stays reproducible.
        existing = (HERE / f"{IDENTIFIER}.installer.yaml").read_text(encoding="utf-8")
        match = re.search(r"^ReleaseDate: (\S+)", existing, re.M)
        release_date = match.group(1) if match else ""

    files = render(version, digest, release_date)

    if check:
        problems = []
        for name, body in files.items():
            path = HERE / name
            if not path.exists():
                problems.append(f"{name} is missing")
            elif path.read_text(encoding="utf-8").replace("\r\n", "\n") != body:
                problems.append(f"{name} is not what the generator produces")
        if problems:
            print("committed winget manifests have drifted:", file=sys.stderr)
            for problem in problems:
                print(f"  {problem}", file=sys.stderr)
            print("run: python packaging/winget/generate.py", file=sys.stderr)
            return 1
        print(f"committed manifests match the generator for {version}")
        return 0

    out.mkdir(parents=True, exist_ok=True)
    for name, body in files.items():
        (out / name).write_text(body, encoding="utf-8")
    print(f"wrote {len(files)} manifests for {version} ({digest[:16]}…) to {out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
