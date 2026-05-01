//! Repository for dependency graph operations.

pub struct DependencyRepository<'a>(&'a rusqlite::Connection);

impl<'a> DependencyRepository<'a> {
    pub fn new(conn: &'a rusqlite::Connection) -> Self {
        Self(conn)
    }

    /// List outgoing dependencies for a repo.
    pub fn list_dependencies(
        &self,
        repo_id: &str,
    ) -> anyhow::Result<Vec<(String, String, f64)>> {
        let mut stmt = self.0.prepare(
            "SELECT to_entity_id, relation_type, confidence FROM relations WHERE from_entity_id = ?1",
        )?;
        let rows = stmt.query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// List incoming (reverse) dependencies for a repo.
    pub fn list_reverse_dependencies(
        &self,
        repo_id: &str,
    ) -> anyhow::Result<Vec<(String, String, f64)>> {
        let mut stmt = self.0.prepare(
            "SELECT from_entity_id, relation_type, confidence FROM relations WHERE to_entity_id = ?1",
        )?;
        let rows = stmt.query_map([repo_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, f64>(2)?,
            ))
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

impl<'a> super::Repository for DependencyRepository<'a> {
    fn conn(&self) -> &rusqlite::Connection {
        self.0
    }
}
