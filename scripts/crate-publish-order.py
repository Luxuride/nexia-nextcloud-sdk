#!/usr/bin/env python3
"""Print the workspace crates in dependency order for a crates.io release.

Workspace members depend on each other by path, so a crate can only be
published after every crate it depends on already exists at the registry.
Output is one crate name per line; standalone crates keep a stable, alphabetical
position within their dependency level.
"""

import json
import subprocess
import sys


def metadata():
    try:
        out = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--no-deps"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"cargo metadata failed: {error}")
    try:
        return json.loads(out.stdout)
    except json.JSONDecodeError as error:
        sys.exit(f"cargo metadata returned invalid JSON: {error}")


def main():
    packages = {p["name"]: p for p in metadata()["packages"]}

    def workspace_dependencies(name):
        return sorted(
            dep["name"] for dep in packages[name]["dependencies"] if dep["name"] in packages
        )

    order = []
    state = {}

    def visit(name):
        mark = state.get(name, 0)
        if mark == 2:
            return
        if mark == 1:
            sys.exit(f"dependency cycle through {name}")
        state[name] = 1
        for dep in workspace_dependencies(name):
            visit(dep)
        state[name] = 2
        order.append(name)

    for name in sorted(packages):
        visit(name)

    print("\n".join(order))


if __name__ == "__main__":
    main()
