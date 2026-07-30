#!/usr/bin/env python3
"""Fail CI when local crates violate the dependency DAG from README.md."""

import json
import subprocess
import sys


ALLOWED_LOCAL_DEPENDENCIES = {
    "wikinext-core": set(),
    "wikinext-store": {"wikinext-core"},
    "wikinext-render": {"wikinext-core"},
    "wikinext-search": {"wikinext-core"},
    "wikinext-app": {
        "wikinext-core",
        "wikinext-store",
        "wikinext-render",
        "wikinext-search",
    },
    "wikinext-server": {"wikinext-app"},
}


def main() -> int:
    metadata = subprocess.run(
        ["cargo", "metadata", "--format-version", "1", "--no-deps", "--locked"],
        check=True,
        capture_output=True,
        text=True,
    )
    packages = json.loads(metadata.stdout)["packages"]
    workspace_names = set(ALLOWED_LOCAL_DEPENDENCIES)
    failures: list[str] = []

    actual_names = {package["name"] for package in packages}
    if actual_names != workspace_names:
        failures.append(
            "workspace crates differ: "
            f"expected={sorted(workspace_names)}, actual={sorted(actual_names)}"
        )

    for package in packages:
        name = package["name"]
        if name not in ALLOWED_LOCAL_DEPENDENCIES:
            continue
        actual = {
            dependency["name"]
            for dependency in package["dependencies"]
            if dependency["name"] in workspace_names
        }
        forbidden = actual - ALLOWED_LOCAL_DEPENDENCIES[name]
        if forbidden:
            failures.append(f"{name} has forbidden local dependencies: {sorted(forbidden)}")

    if failures:
        for failure in failures:
            print(f"architecture error: {failure}", file=sys.stderr)
        return 1

    print("crate dependency architecture: OK")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
