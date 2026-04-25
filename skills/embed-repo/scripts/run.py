#!/usr/bin/env python3
"""Entry point for embed-repo skill."""
import argparse
import os
import subprocess
import sys


def main():
    parser = argparse.ArgumentParser(description="Generate embeddings for a repository")
    parser.add_argument("--repo-id", required=True, help="Target repository ID")
    parser.add_argument("--device", default="auto", help="Device (cpu, cuda, auto)")
    args = parser.parse_args()

    # Locate local.py relative to devbase workspace
    devbase_home = os.environ.get("DEVBASE_HOME", "")
    local_py = os.path.join(devbase_home, "..", "..", "tools", "embedding-provider", "local.py")
    if not os.path.exists(local_py):
        # Fallback: search in common locations
        candidates = [
            os.path.join(os.path.dirname(__file__), "..", "..", "..", "tools", "embedding-provider", "local.py"),
            os.path.join(os.path.dirname(__file__), "..", "..", "tools", "embedding-provider", "local.py"),
        ]
        for c in candidates:
            if os.path.exists(c):
                local_py = os.path.abspath(c)
                break

    if not os.path.exists(local_py):
        print(f"ERROR: Could not find local.py embedding provider", file=sys.stderr)
        sys.exit(1)

    cmd = [
        sys.executable, local_py,
        "--repo-id", args.repo_id,
        "--device", args.device,
    ]
    print(f"Running: {' '.join(cmd)}")
    result = subprocess.run(cmd)
    sys.exit(result.returncode)


if __name__ == "__main__":
    main()
