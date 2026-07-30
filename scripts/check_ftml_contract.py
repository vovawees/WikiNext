#!/usr/bin/env python3
"""Verify the FTML version and feature surface selected by the workspace."""

import json
import subprocess
import sys


EXPECTED_VERSION = "1.41.0"
EXPECTED_FEATURES = {"html", "lightningcss"}


def main() -> int:
    metadata = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--locked"],
        check=True,
        capture_output=True,
        text=True,
    )
    document = json.loads(metadata.stdout)
    packages = {package["id"]: package for package in document["packages"]}
    nodes = [
        node
        for node in document["resolve"]["nodes"]
        if packages[node["id"]]["name"] == "ftml"
    ]

    if len(nodes) != 1:
        print(
            f"FTML contract error: expected one resolved package, found {len(nodes)}",
            file=sys.stderr,
        )
        return 1

    node = nodes[0]
    package = packages[node["id"]]
    actual_features = set(node["features"])
    failures: list[str] = []

    if package["version"] != EXPECTED_VERSION:
        failures.append(
            f"expected version {EXPECTED_VERSION}, found {package['version']}"
        )
    if actual_features != EXPECTED_FEATURES:
        failures.append(
            "expected features "
            f"{sorted(EXPECTED_FEATURES)}, found {sorted(actual_features)}"
        )

    if failures:
        for failure in failures:
            print(f"FTML contract error: {failure}", file=sys.stderr)
        return 1

    print(
        "FTML contract: "
        f"{EXPECTED_VERSION}, features={sorted(EXPECTED_FEATURES)}: OK"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
