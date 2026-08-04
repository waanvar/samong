"""Write the .mcpb zip, with the permission bits the binaries need.

Python rather than the `zip` command for two reasons. It exists on every runner
and on Windows, so the bundle can be built and inspected anywhere. And it makes
the mode bits explicit: a zip entry carries the Unix permissions in its external
attributes, and a server binary that unpacks without the execute bit cannot be
launched — a failure that would surface only on someone else's machine, after
install, with nothing to explain it.

Also verifies the result, because a packaging step with no assertions ships
whatever it happened to produce.

Usage: pack.py <staging dir> <output .mcpb>
"""

import json
import stat
import sys
import zipfile
from pathlib import Path

EXECUTABLE = 0o755
REGULAR = 0o644

# What every bundle must contain: the manifest at the root, and one server binary
# per platform the manifest promises to support.
REQUIRED = [
    "manifest.json",
    "server/samong-mcp",  # linux
    "server/samong-mcp.exe",  # windows
    "server/samong-mcp-macos",  # macOS, universal
]


def add(zf: zipfile.ZipFile, path: Path, arcname: str, mode: int) -> None:
    info = zipfile.ZipInfo(arcname)
    # A fixed timestamp so the same inputs give the same bytes; a bundle whose
    # hash changes without its content changing cannot be reasoned about.
    info.date_time = (1980, 1, 1, 0, 0, 0)
    info.compress_type = zipfile.ZIP_DEFLATED
    info.external_attr = (stat.S_IFREG | mode) << 16
    zf.writestr(info, path.read_bytes())


def main() -> int:
    staging = Path(sys.argv[1])
    out = Path(sys.argv[2])

    manifest = staging / "manifest.json"
    parsed = json.loads(manifest.read_text(encoding="utf-8"))
    for field in ("manifest_version", "name", "version", "description", "author", "server"):
        if field not in parsed:
            print(f"manifest.json is missing {field}", file=sys.stderr)
            return 1
    if parsed["server"].get("type") != "binary":
        print("this bundle ships a compiled binary; server.type must be 'binary'", file=sys.stderr)
        return 1

    out.parent.mkdir(parents=True, exist_ok=True)
    if out.exists():
        out.unlink()

    # Sorted, so the archive order does not depend on directory iteration.
    with zipfile.ZipFile(out, "w") as zf:
        add(zf, manifest, "manifest.json", REGULAR)
        for path in sorted((staging / "server").iterdir()):
            if path.is_file():
                add(zf, path, f"server/{path.name}", EXECUTABLE)

    with zipfile.ZipFile(out) as zf:
        names = zf.namelist()
        for required in REQUIRED:
            if required not in names:
                print(f"{required} is missing from the bundle", file=sys.stderr)
                return 1
        for name in names:
            if name == "manifest.json":
                continue
            mode = (zf.getinfo(name).external_attr >> 16) & 0o777
            if mode != EXECUTABLE:
                print(f"{name} is not executable in the bundle ({oct(mode)})", file=sys.stderr)
                return 1

    print(f"packed {out} ({out.stat().st_size} bytes, {len(names)} entries)")
    for name in names:
        mode = (zipfile.ZipFile(out).getinfo(name).external_attr >> 16) & 0o777
        print(f"  {oct(mode)[2:]}  {name}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
