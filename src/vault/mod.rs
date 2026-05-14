// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
pub mod backlinks;
pub mod export;
pub mod frontmatter;
pub mod fs_io;
pub mod indexer;
pub mod scanner;
pub mod wikilink;

use crate::storage::AppContext;

impl crate::clients::VaultClient for AppContext {
    fn list_vault_notes(&self) -> anyhow::Result<serde_json::Value> {
        let conn = self.conn()?;
        let notes = crate::registry::vault::list_vault_notes(&conn)?;
        let results: Vec<serde_json::Value> = notes
            .into_iter()
            .map(|n| {
                serde_json::json!({
                    "id": n.id,
                    "path": n.path,
                    "title": n.title,
                    "tags": n.tags,
                })
            })
            .collect();
        Ok(serde_json::json!({"success": true, "count": results.len(), "notes": results}))
    }

    fn read_vault_note(&self, path: &str) -> anyhow::Result<serde_json::Value> {
        let (body, frontmatter) = fs_io::read_note_body(path)
            .ok_or_else(|| anyhow::anyhow!("note not found or unreadable"))?;
        Ok(serde_json::json!({
            "success": true,
            "path": path,
            "content": body,
            "frontmatter": frontmatter,
        }))
    }

    fn get_backlinks(&self, note_id: &str) -> anyhow::Result<serde_json::Value> {
        let vault_dir = self.storage.workspace_dir().ok().map(|ws| ws.join("vault"));
        let backlinks = if let Some(vd) = vault_dir {
            match backlinks::build_backlink_index(&vd) {
                Ok(index) => backlinks::get_backlinks(&index, note_id),
                Err(_) => Vec::new(),
            }
        } else {
            Vec::new()
        };
        Ok(serde_json::json!({
            "success": true,
            "target": note_id,
            "count": backlinks.len(),
            "backlinks": backlinks,
        }))
    }

    fn build_vault_graph(&self, repo_id: Option<&str>) -> anyhow::Result<serde_json::Value> {
        let vault_dir = self.storage.workspace_dir().ok().map(|ws| ws.join("vault"));
        let Some(vd) = vault_dir else {
            return Ok(serde_json::json!({
                "success": true,
                "count": 0,
                "edge_count": 0,
                "nodes": [],
                "edges": [],
            }));
        };

        let index = backlinks::build_backlink_index(&vd)?;

        let mut id_to_title: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut id_to_repo: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();

        for entry in walkdir::WalkDir::new(&vd)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| e.path().extension().map(|ext| ext == "md").unwrap_or(false))
        {
            let path = entry.path();
            let rel_path = path.strip_prefix(&vd).unwrap_or(path);
            let id = rel_path.to_string_lossy().replace('\\', "/");

            let content = match std::fs::read_to_string(path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some((fm, _)) = frontmatter::extract_frontmatter(&content) {
                id_to_title.insert(id.clone(), fm.title.unwrap_or_else(|| id.clone()));
                if let Some(repo) = fm.repo {
                    id_to_repo.insert(id, repo);
                }
            } else {
                id_to_title.insert(id.clone(), id.clone());
            }
        }

        let allowed_ids: std::collections::HashSet<String> = if let Some(rid) = repo_id {
            id_to_repo.iter().filter(|(_, r)| *r == rid).map(|(id, _)| id.clone()).collect()
        } else {
            id_to_title.keys().cloned().collect()
        };

        let mut id_lookup: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for id in id_to_title.keys() {
            id_lookup.insert(id.clone(), id.clone());
            if let Some(stem) = id.strip_suffix(".md") {
                id_lookup.insert(stem.to_string(), id.clone());
            }
        }

        let nodes: Vec<_> = allowed_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "id": id,
                    "title": id_to_title.get(id).unwrap_or(id),
                })
            })
            .collect();

        let mut edges = Vec::new();
        for (target, sources) in &index {
            let normalized = id_lookup.get(target).cloned().unwrap_or_else(|| target.clone());
            if !allowed_ids.contains(&normalized) {
                continue;
            }
            for source in sources {
                if allowed_ids.contains(source) {
                    edges.push(serde_json::json!({
                        "source": source,
                        "target": &normalized,
                    }));
                }
            }
        }

        Ok(serde_json::json!({
            "success": true,
            "count": nodes.len(),
            "edge_count": edges.len(),
            "nodes": nodes,
            "edges": edges,
        }))
    }

    fn export_vault(&self, output_dir: &str) -> anyhow::Result<serde_json::Value> {
        let vault_dir = self.storage.workspace_dir()?.join("vault");
        let out = std::path::PathBuf::from(output_dir);
        crate::vault::export::export_vault(&vault_dir, &out)
    }
}
