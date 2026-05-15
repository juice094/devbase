#!/usr/bin/env python3
"""Update README.md release asset metadata.

Usage:
    python scripts/update_readme.py --version v0.21.0 \
        --windows-size 8.9 --linux-size 8.8
"""
import argparse
import re
import sys


def main():
    parser = argparse.ArgumentParser(description="Update README release metadata")
    parser.add_argument("--version", required=True, help="New version tag (e.g. v0.21.0)")
    parser.add_argument("--windows-size", type=float, required=True, help="Windows asset size in MB")
    parser.add_argument("--linux-size", type=float, required=True, help="Linux asset size in MB")
    parser.add_argument("--readme", default="README.md", help="Path to README.md")
    args = parser.parse_args()

    with open(args.readme, "r", encoding="utf-8") as f:
        content = f.read()

    old_version_pattern = r"devbase-v[\d.]+"
    new_version_str = f"devbase-{args.version}"

    # Replace version references in filenames and URLs
    content = re.sub(old_version_pattern, new_version_str, content)

    # Replace version in directory names (e.g. devbase-v0.20.0-linux-x64)
    def repl_dir(m):
        return f"devbase-{args.version}-{m.group(1)}-x64"
    content = re.sub(
        r"devbase-v[\d.]+-(linux|windows)-x64",
        repl_dir,
        content,
    )

    # Replace version in release download URLs (/download/v0.20.0/)
    content = re.sub(
        r"/download/v[\d.]+/",
        f"/download/{args.version}/",
        content,
    )

    # Replace size in table
    content = re.sub(
        r"(\| Windows x86_64 \| .*? \| )~[\d.]+ MB( \|)",
        f"\1~{args.windows_size:.1f} MB\2",
        content,
    )
    content = re.sub(
        r"(\| Linux x86_64 \| .*? \| )~[\d.]+ MB( \|)",
        f"\1~{args.linux_size:.1f} MB\2",
        content,
    )

    with open(args.readme, "w", encoding="utf-8") as f:
        f.write(content)

    print(f"Updated {args.readme} for version {args.version}")


if __name__ == "__main__":
    main()
