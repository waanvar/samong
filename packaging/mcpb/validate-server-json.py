"""Validate server.json against the registry's published schema.

Written after v0.3.7's registry publish was rejected for a `description` of 168
characters against a limit of 100. The limit was in the schema all along —
`ServerDetail.properties.description.maxLength` — and was missed because the
schema had only been inspected for enums. Reading a schema by eye is not
validating against it.

Fetches the schema named by `$schema` in the document itself, so the check follows
whatever version the file claims rather than one hard-coded here.

Usage: validate-server-json.py <server.json> [--require-published]
Exit 0 when valid; 1 with the offending field and value when not.
"""

import json
import re
import sys
import urllib.parse
import urllib.request

TIMEOUT = 30
SHA256 = re.compile(r"^[a-f0-9]{64}$")


def fetch(url: str) -> dict:
    with urllib.request.urlopen(url, timeout=TIMEOUT) as response:  # noqa: S310
        return json.loads(response.read())


def resolve(schema: dict, node: dict) -> dict:
    """Follow a local $ref one level; the registry schema nests no deeper."""
    ref = node.get("$ref")
    if not ref or not ref.startswith("#/definitions/"):
        return node
    return schema["definitions"][ref.split("/")[-1]]


def check(schema: dict, node: dict, value, path: str, problems: list) -> None:
    node = resolve(schema, node)

    if "enum" in node and value not in node["enum"]:
        problems.append(f"{path}: {value!r} is not one of {node['enum']}")
    if isinstance(value, str):
        if "maxLength" in node and len(value) > node["maxLength"]:
            problems.append(
                f"{path}: {len(value)} characters, limit is {node['maxLength']}\n"
                f"    {value!r}"
            )
        if "minLength" in node and len(value) < node["minLength"]:
            problems.append(f"{path}: shorter than {node['minLength']}")
        if "pattern" in node and not re.search(node["pattern"], value):
            problems.append(f"{path}: {value!r} does not match {node['pattern']}")

    if isinstance(value, dict):
        for required in node.get("required", []):
            if required not in value:
                problems.append(f"{path}.{required} is required and missing")
        for key, child in value.items():
            declared = node.get("properties", {}).get(key)
            if declared is not None:
                check(schema, declared, child, f"{path}.{key}", problems)
    elif isinstance(value, list) and "items" in node:
        for index, item in enumerate(value):
            check(schema, node["items"], item, f"{path}[{index}]", problems)


# Rules the registry enforces in code but the schema cannot express — taken from
# `internal/validators/registries/mcpb.go` in the registry repository, because
# discovering them one rejected release at a time is not a method. v0.3.7 was
# rejected for the description length, v0.3.8 for carrying `registryBaseUrl`.
GITHUB_RELEASE = re.compile(r"^/[^/]+/[^/]+/releases/download/[^/]+/[^/]+$")
ALLOWED_HOSTS = ("github.com", "www.github.com", "gitlab.com", "www.gitlab.com")


def check_mcpb(package: dict, require_published: bool, problems: list) -> None:
    url = package.get("identifier", "")
    if not url:
        problems.append("packages[0].identifier is required for MCPB packages")
        return

    if "registryBaseUrl" in package:
        problems.append(
            "packages[0].registryBaseUrl must be absent for MCPB — the full "
            "download URL goes in identifier"
        )

    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != "https":
        problems.append(f"packages[0].identifier must use HTTPS: {url}")
    if "mcp" not in url:
        problems.append(f"packages[0].identifier must contain 'mcp': {url}")
    if parsed.hostname and parsed.hostname.lower() not in ALLOWED_HOSTS:
        problems.append(
            f"MCPB bundles must be hosted on GitHub or GitLab, not {parsed.hostname}"
        )
    if parsed.hostname and "github.com" in parsed.hostname.lower():
        if not GITHUB_RELEASE.match(parsed.path):
            problems.append(
                "a GitHub MCPB URL must look like "
                f"/owner/repo/releases/download/tag/filename, not {parsed.path}"
            )

    # The committed file deliberately has no hash — it would be a claim about a
    # file that does not exist. At release time, after the upload, both the hash
    # and a reachable URL are required.
    if require_published:
        if not SHA256.match(package.get("fileSha256", "")):
            problems.append("packages[0].fileSha256 is required when publishing")
        try:
            request = urllib.request.Request(url, method="HEAD")
            with urllib.request.urlopen(request, timeout=TIMEOUT) as response:  # noqa: S310
                if response.status != 200:
                    problems.append(f"{url} returned {response.status}")
        except Exception as error:
            problems.append(f"the registry will fetch {url} and it is not reachable: {error}")


def main() -> int:
    path = sys.argv[1]
    require_published = "--require-published" in sys.argv[2:]
    # utf-8-sig, not utf-8: PowerShell's `Set-Content -Encoding utf8` writes a BOM
    # on Windows, and a validator that refuses the file for a byte the author
    # cannot see is a validator that gets ignored.
    with open(path, encoding="utf-8-sig") as f:
        doc = json.load(f)

    url = doc.get("$schema")
    if not url:
        print(f"{path} declares no $schema", file=sys.stderr)
        return 1

    try:
        schema = fetch(url)
    except Exception as error:  # offline, or the schema moved
        print(f"could not fetch {url}: {error}", file=sys.stderr)
        return 1

    root = resolve(schema, schema)
    problems: list = []
    check(schema, root, doc, "server", problems)

    for index, package in enumerate(doc.get("packages", [])):
        if package.get("registryType") == "mcpb":
            check_mcpb(package, require_published, problems)
        elif index == 0:
            problems.append(
                f"packages[{index}]: only mcpb is handled here; add rules before "
                "publishing another type"
            )

    if problems:
        print(f"{path} does not satisfy {url}:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        return 1

    print(f"{path} is valid against {url}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
