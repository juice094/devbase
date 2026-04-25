#!/usr/bin/env python3
"""Entry point for search-workspace skill."""
import argparse
import json
import os
import sqlite3
import struct
import sys


def f32_blob_to_vec(blob: bytes) -> list[float]:
    return list(struct.unpack(f"<{len(blob) // 4}f", blob))


def main():
    parser = argparse.ArgumentParser(description="Search workspace symbols")
    parser.add_argument("--query", required=True, help="Search query")
    parser.add_argument("--limit", type=int, default=10, help="Max results")
    parser.add_argument("--repo-id", default="", help="Specific repo ID")
    args = parser.parse_args()

    db_path = os.environ.get("DEVBASE_REGISTRY_PATH", "")
    if not db_path or not os.path.exists(db_path):
        print(f"ERROR: Registry DB not found at {db_path}", file=sys.stderr)
        sys.exit(1)

    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()

    # Simple keyword search fallback (no embedding in skill context yet)
    if args.repo_id:
        cursor.execute(
            "SELECT repo_id, file_path, name, symbol_type, line_start, signature "
            "FROM code_symbols WHERE repo_id = ? AND (name LIKE ? OR signature LIKE ?) "
            "LIMIT ?",
            (args.repo_id, f"%{args.query}%", f"%{args.query}%", args.limit)
        )
    else:
        cursor.execute(
            "SELECT repo_id, file_path, name, symbol_type, line_start, signature "
            "FROM code_symbols WHERE name LIKE ? OR signature LIKE ? "
            "LIMIT ?",
            (f"%{args.query}%", f"%{args.query}%", args.limit)
        )

    rows = cursor.fetchall()
    results = []
    for row in rows:
        results.append({
            "repo_id": row[0],
            "file": row[1],
            "name": row[2],
            "type": row[3],
            "line": row[4],
            "signature": row[5],
        })

    print(json.dumps({"results": results, "count": len(results)}, indent=2))
    conn.close()


if __name__ == "__main__":
    main()
