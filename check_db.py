import sqlite3
import os

db_path = os.path.expandvars(r"$LOCALAPPDATA\devbase\registry.db")
conn = sqlite3.connect(db_path)
c = conn.cursor()

print("user_version:", c.execute("PRAGMA user_version").fetchone()[0])

c.execute("SELECT name FROM sqlite_master WHERE type='table'")
tables = [row[0] for row in c.fetchall()]
print("tables:", tables)

for t in tables:
    try:
        count = c.execute(f"SELECT COUNT(*) FROM {t}").fetchone()[0]
        print(f"  {t}: {count}")
    except Exception as e:
        print(f"  {t}: ERROR {e}")

c.execute("SELECT repo_id, last_commit_hash, indexed_at FROM repo_index_state")
print("repo_index_state:", c.fetchall())

conn.close()
