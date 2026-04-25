#!/usr/bin/env python3
"""Entry point for knowledge-report skill."""
import argparse
import os
import sqlite3
import sys
from datetime import datetime


def main():
    parser = argparse.ArgumentParser(description="Generate knowledge report")
    parser.add_argument("--repo-id", required=True, help="Target repository ID")
    args = parser.parse_args()

    db_path = os.environ.get("DEVBASE_REGISTRY_PATH", "")
    if not db_path or not os.path.exists(db_path):
        print(f"ERROR: Registry DB not found at {db_path}", file=sys.stderr)
        sys.exit(1)

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    # Repo metadata
    cursor.execute(
        "SELECT id, local_path, language, workspace_type, data_tier, last_synced_at, stars FROM repos WHERE id = ?",
        (args.repo_id,)
    )
    repo = cursor.fetchone()
    if not repo:
        print(f"ERROR: Repository '{args.repo_id}' not found", file=sys.stderr)
        sys.exit(1)

    # Tags
    cursor.execute("SELECT tag FROM repo_tags WHERE repo_id = ?", (args.repo_id,))
    tags = [r[0] for r in cursor.fetchall()]

    # Metrics
    cursor.execute(
        "SELECT total_lines, source_lines, test_lines, file_count, language_breakdown FROM repo_code_metrics WHERE repo_id = ?",
        (args.repo_id,)
    )
    metrics = cursor.fetchone()

    # Symbols count
    cursor.execute("SELECT COUNT(*) FROM code_symbols WHERE repo_id = ?", (args.repo_id,))
    symbol_count = cursor.fetchone()[0]

    # Embeddings count
    cursor.execute("SELECT COUNT(*) FROM code_embeddings WHERE repo_id = ?", (args.repo_id,))
    embedding_count = cursor.fetchone()[0]

    print(f"# Knowledge Report: {repo['id']}")
    print(f"\nGenerated: {datetime.utcnow().isoformat()}Z")
    print(f"\n## Metadata")
    print(f"- **Path**: {repo['local_path']}")
    print(f"- **Language**: {repo['language'] or 'Unknown'}")
    print(f"- **Workspace Type**: {repo['workspace_type']}")
    print(f"- **Data Tier**: {repo['data_tier']}")
    print(f"- **Tags**: {', '.join(tags) if tags else 'None'}")
    print(f"- **Stars**: {repo['stars'] or 'N/A'}")
    print(f"- **Last Synced**: {repo['last_synced_at'] or 'Never'}")

    print(f"\n## Code Metrics")
    if metrics:
        print(f"- **Total Lines**: {metrics['total_lines'] or 'N/A'}")
        print(f"- **Source Lines**: {metrics['source_lines'] or 'N/A'}")
        print(f"- **Test Lines**: {metrics['test_lines'] or 'N/A'}")
        print(f"- **File Count**: {metrics['file_count'] or 'N/A'}")
        if metrics['language_breakdown']:
            print(f"- **Language Breakdown**: {metrics['language_breakdown']}")
    else:
        print("- No metrics available. Run `devbase index` first.")

    print(f"\n## Symbol Coverage")
    print(f"- **Code Symbols**: {symbol_count}")
    print(f"- **Embeddings**: {embedding_count} ({(embedding_count / max(symbol_count, 1) * 100):.1f}% coverage)")

    conn.close()


if __name__ == "__main__":
    main()
