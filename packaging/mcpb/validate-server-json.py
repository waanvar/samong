"""Validate server.json against the registry's published schema.

Written after v0.3.7's registry publish was rejected for a `description` of 168
characters against a limit of 100. The limit was in the schema all along —
`ServerDetail.properties.description.maxLength` — and was missed because the
schema had only been inspected for enums. Reading a schema by eye is not
validating against it.

Fetches the schema named by `$schema` in the document itself, so the check follows
whatever version the file claims rather than one hard-coded here.

Usage: validate-server-json.py <server.json>
Exit 0 when valid; 1 with the offending field and value when not.
"""

import json
import re
import sys
import urllib.request

TIMEOUT = 30


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


def main() -> int:
    path = sys.argv[1]
    with open(path, encoding="utf-8") as f:
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

    if problems:
        print(f"{path} does not satisfy {url}:", file=sys.stderr)
        for problem in problems:
            print(f"  {problem}", file=sys.stderr)
        return 1

    print(f"{path} is valid against {url}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
