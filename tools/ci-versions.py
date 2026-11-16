#!/usr/bin/env python3

import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LINUX = ROOT / "packaging/linux/package.sh"


def fetch(url: str) -> str:
    request = urllib.request.Request(url, headers={"User-Agent": "RimStack ci-versions"})
    with urllib.request.urlopen(request, timeout=30) as answer:
        return answer.read().decode()


def newest_release(repository: str) -> str:
    releases = json.loads(fetch(f"https://api.github.com/repos/{repository}/releases?per_page=20"))
    tags = [r["tag_name"] for r in releases if not r["draft"] and r["tag_name"] != "continuous"]
    return tags[0]


def main() -> int:
    check = "--check" in sys.argv[1:]
    linux = LINUX.read_text()
    moved = []

    for repository in ["linuxdeploy/linuxdeploy"]:
        pattern = rf"github\.com/{repository}/releases/download/([^/]+)/"
        now = re.search(pattern, linux).group(1)
        newest = newest_release(repository)
        if newest != now:
            linux = linux.replace(f"{repository}/releases/download/{now}/", f"{repository}/releases/download/{newest}/")
            moved.append(f"{repository.split('/')[1]} {now} → {newest}")

    print("\n".join(moved) if moved else "everything is current")
    if not check:
        LINUX.write_text(linux)
    return 0


if __name__ == "__main__":
    sys.exit(main())
