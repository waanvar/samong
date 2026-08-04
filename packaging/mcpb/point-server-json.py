"""Point server.json at a bundle that now exists.

The committed `server.json` carries everything stable about the registry entry and
deliberately omits `fileSha256`: a hash in git would be a claim about a file that
has not been built yet, and the version would go stale the moment it is bumped.
The release workflow fills both in from the artefact it just uploaded.

A script rather than inline YAML because it needs to be runnable — and testable —
outside a workflow run.

Usage: point-server-json.py <server.json> <version> <sha256> <url>
"""

import json
import re
import sys

SHA256 = re.compile(r"^[a-f0-9]{64}$")


def main() -> int:
    path, version, sha, url = sys.argv[1:5]

    if not SHA256.match(sha):
        print(f"not a sha256 digest: {sha!r}", file=sys.stderr)
        return 1
    # The registry requires "mcp" somewhere in the URL. Checked here so a rename
    # fails the release rather than the publish.
    if "mcp" not in url:
        print(f"the bundle URL must contain 'mcp': {url}", file=sys.stderr)
        return 1
    if version not in url:
        print(f"the URL does not name version {version}: {url}", file=sys.stderr)
        return 1

    with open(path, encoding="utf-8") as f:
        doc = json.load(f)

    doc["version"] = version
    if len(doc["packages"]) != 1:
        print("expected exactly one package entry", file=sys.stderr)
        return 1
    package = doc["packages"][0]
    package["version"] = version
    package["identifier"] = url
    package["fileSha256"] = sha

    with open(path, "w", encoding="utf-8") as f:
        json.dump(doc, f, indent=2, ensure_ascii=False)
        f.write("\n")

    print(json.dumps(doc, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
