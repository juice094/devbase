import sqlite3
conn = sqlite3.connect(r'C:\Users\22414\AppData\Local\devbase\registry.db')
c = conn.execute('PRAGMA user_version')
print('user_version:', c.fetchone()[0])
c = conn.execute("SELECT repo_id, last_commit_hash, indexed_at FROM repo_index_state WHERE repo_id='devbase'")
print('repo_index_state:', c.fetchone())
conn.close()
