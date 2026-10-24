#!/usr/bin/env python3
"""Ensure every workspace crate has a crates.io trusted-publishing config.

crates.io only accepts a trusted-publishing config for a crate that already
exists, so this is run repeatedly: crates created later are picked up on the
next pass. The token is read from the cargo credentials file and never printed.

Usage: sync-trusted-publishing.py [--until-complete]
"""

import argparse
import json
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from pathlib import Path

REGISTRY = "https://crates.io"
OWNER = "Luxuride"
REPOSITORY = "nextcloud-rust-sdk"
WORKFLOW = "release.yml"
USER_AGENT = "nextcloud-rust-sdk-release-setup"
CREDENTIALS = Path("/usr/local/cargo/credentials.toml")


def registry_token():
    try:
        text = CREDENTIALS.read_text()
    except OSError as error:
        sys.exit(f"cannot read {CREDENTIALS}: {error}")
    for line in text.splitlines():
        key, _, value = line.partition("=")
        if key.strip() == "token":
            token = value.strip().strip('"')
            if token:
                return token
    sys.exit(f"no token in {CREDENTIALS}")


def api_url(path):
    url = urllib.parse.urlparse(f"{REGISTRY}{path}")
    if url.scheme != "https" or url.netloc != "crates.io":
        sys.exit(f"refusing non-crates.io URL: {url.geturl()}")
    return url.geturl()


def request(method, path, token, payload=None):
    # api_url() pins the scheme and host, so the request never leaves crates.io.
    data = None if payload is None else json.dumps(payload).encode()
    req = urllib.request.Request(  # noqa: S310  # nosemgrep
        api_url(path),
        data=data,
        method=method,
        headers={
            "Authorization": token,
            "Content-Type": "application/json",
            "User-Agent": USER_AGENT,
        },
    )
    try:
        with urllib.request.urlopen(req) as response:  # noqa: S310  # nosemgrep
            return response.status, json.loads(response.read() or b"{}")
    except urllib.error.HTTPError as error:
        body = error.read()
        try:
            return error.code, json.loads(body or b"{}")
        except json.JSONDecodeError:
            return error.code, {"errors": [{"detail": body.decode(errors="replace")}]}
    except urllib.error.URLError as error:
        return 0, {"errors": [{"detail": str(error)}]}


def workspace_crates():
    try:
        out = subprocess.run(
            ["python3", "scripts/crate-publish-order.py"],
            check=True,
            capture_output=True,
            text=True,
        )
    except (OSError, subprocess.CalledProcessError) as error:
        sys.exit(f"cannot list workspace crates: {error}")
    return [line for line in out.stdout.splitlines() if line]


def has_config(token, crate):
    status, body = request(
        "GET",
        f"/api/v1/trusted_publishing/github_configs?crate={crate}",
        token,
    )
    if status != 200:
        return None
    return bool(body.get("github_configs"))


def create_config(token, crate):
    status, body = request(
        "POST",
        "/api/v1/trusted_publishing/github_configs",
        token,
        {
            "github_config": {
                "crate": crate,
                "repository_owner": OWNER,
                "repository_name": REPOSITORY,
                "workflow_filename": WORKFLOW,
                "environment": None,
            }
        },
    )
    if status == 200:
        return "created", None
    details = body.get("errors", [{}])[0].get("detail", "")
    if status == 409:
        return "exists", details
    if "does not exist" in details:
        return "missing", details
    return "error", f"HTTP {status}: {details}"


def sync(token, crates):
    pending = []
    for crate in crates:
        configured = has_config(token, crate)
        if configured:
            print(f"{crate}: already configured")
            continue
        if configured is None:
            print(f"{crate}: cannot read configs, will retry")
            pending.append(crate)
            continue
        result, detail = create_config(token, crate)
        if result == "created":
            print(f"{crate}: configured")
        elif result == "exists":
            print(f"{crate}: already configured")
        elif result == "missing":
            print(f"{crate}: not on crates.io yet")
            pending.append(crate)
        else:
            print(f"{crate}: failed ({detail})")
            pending.append(crate)
        time.sleep(1)
    return pending


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--until-complete",
        action="store_true",
        help="keep retrying until every crate is configured",
    )
    args = parser.parse_args()

    token = registry_token()
    crates = workspace_crates()
    while True:
        pending = sync(token, crates)
        if not pending:
            print("all crates have a trusted-publishing config")
            return 0
        message = f"{len(pending)} crate(s) pending: {' '.join(pending)}"
        if not args.until_complete:
            print(message)
            return 1
        print(f"{message}; retrying in 60s")
        time.sleep(60)


if __name__ == "__main__":
    sys.exit(main())
