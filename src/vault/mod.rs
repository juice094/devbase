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
        let backlinks = match self.conn() {
            Ok(conn) => match crate::registry::vault::list_vault_notes(&conn) {
                Ok(notes) => notes
                    .into_iter()
                    .filter(|n| {
                        n.outgoing_links.iter().any(|l| {
                            let normalized = l.replace('\\', "/");
                            normalized == note_id.replace('\\', "/")
                                || normalized
                                    == note_id.replace('\\', "/").strip_suffix(".md").unwrap_or(&note_id.replace('\\', "/"))
                                || l == note_id
                        })
                    })
                    .map(|n| n.id.replace('\\', "/"))
                    .collect(),
                Err(_) => Vec::new(),
            },
            Err(_) => Vec::new(),
        };
        Ok(serde_json::json!({
            "success": true,
            "target": note_id,
            "count": backlinks.len(),
            "backlinks": backlinks,
        }))
    }

    fn build_vault_graph(
        &self,
        repo_id: Option<&str>,
        note_id: Option<&str>,
        depth: usize,
    ) -> anyhow::Result<serde_json::Value> {
        let conn = match self.conn() {
            Ok(c) => c,
            Err(_) => {
                return Ok(serde_json::json!({
                    "success": true,
                    "count": 0,
                    "edge_count": 0,
                    "nodes": [],
                    "edges": [],
                }));
            }
        };

        let notes = match crate::registry::vault::list_vault_notes(&conn) {
            Ok(n) => n,
            Err(_) => {
                return Ok(serde_json::json!({
                    "success": true,
                    "count": 0,
                    "edge_count": 0,
                    "nodes": [],
                    "edges": [],
                }));
            }
        };

        if notes.is_empty() {
            return Ok(serde_json::json!({
                "success": true,
                "count": 0,
                "edge_count": 0,
                "nodes": [],
                "edges": [],
            }));
        }

        let mut id_to_title: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut id_to_repo: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut outgoing: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        let mut incoming: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();

        for note in &notes {
            let id = note.id.replace('\\', "/");
            id_to_title
                .insert(id.clone(), note.title.clone().unwrap_or_else(|| id.clone()));
            if let Some(ref r) = note.linked_repo {
                id_to_repo.insert(id.clone(), r.clone());
            }

            let targets: Vec<String> = note
                .outgoing_links
                .iter()
                .map(|t| t.replace('\\', "/"))
                .collect();
            outgoing.insert(id.clone(), targets.clone());

            for target in targets {
                incoming
                    .entry(target.clone())
                    .or_default()
                    .push(id.clone());
                if let Some(stem) = target.strip_suffix(".md") {
                    incoming
                        .entry(stem.to_string())
                        .or_default()
                        .push(id.clone());
                }
            }
        }

        let mut id_lookup: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        for id in id_to_title.keys() {
            id_lookup.insert(id.clone(), id.clone());
            if let Some(stem) = id.strip_suffix(".md") {
                id_lookup.insert(stem.to_string(), id.clone());
            }
        }

        let allowed_ids: std::collections::HashSet<String> = if let Some(rid) = repo_id {
            id_to_repo
                .iter()
                .filter(|(_, r)| *r == rid)
                .map(|(id, _)| id.clone())
                .collect()
        } else {
            id_to_title.keys().cloned().collect()
        };

        let max_depth = depth.max(1).min(3);

        let (selected_nodes, selected_edges): (
            std::collections::HashSet<String>,
            Vec<(String, String)>,
        ) = if let Some(start_id) = note_id {
            let start_normalized = id_lookup
                .get(start_id)
                .cloned()
                .unwrap_or_else(|| start_id.replace('\\', "/"));
            if !allowed_ids.contains(&start_normalized) {
                return Ok(serde_json::json!({
                    "success": true,
                    "count": 1,
                    "edge_count": 0,
                    "nodes": [serde_json::json!({
                        "id": start_normalized,
                        "title": id_to_title.get(&start_normalized).unwrap_or(&start_normalized),
                    })],
                    "edges": [],
                }));
            }

            let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
            let mut edges: Vec<(String, String)> = Vec::new();
            let mut queue: Vec<(String, usize)> = vec![(start_normalized.clone(), 0)];
            visited.insert(start_normalized.clone());

            while let Some((current, dist)) = queue.pop() {
                if dist >= max_depth {
                    continue;
                }
                for target in outgoing.get(&current).into_iter().flatten() {
                    let norm = id_lookup
                        .get(target)
                        .cloned()
                        .unwrap_or_else(|| target.clone());
                    if allowed_ids.contains(&norm) {
                        edges.push((current.clone(), norm.clone()));
                        if visited.insert(norm.clone()) {
                            queue.push((norm, dist + 1));
                        }
                    }
                }
                for source in incoming.get(&current).into_iter().flatten() {
                    let norm = id_lookup
                        .get(source)
                        .cloned()
                        .unwrap_or_else(|| source.clone());
                    if allowed_ids.contains(&norm) {
                        edges.push((norm.clone(), current.clone()));
                        if visited.insert(norm.clone()) {
                            queue.push((norm, dist + 1));
                        }
                    }
                }
            }

            (visited, edges)
        } else {
            let mut all_edges: Vec<(String, String)> = Vec::new();
            for (source, targets) in &outgoing {
                if !allowed_ids.contains(source) {
                    continue;
                }
                for target in targets {
                    let norm = id_lookup
                        .get(target)
                        .cloned()
                        .unwrap_or_else(|| target.clone());
                    if allowed_ids.contains(&norm) {
                        all_edges.push((source.clone(), norm.clone()));
                    }
                }
            }
            (allowed_ids.clone(), all_edges)
        };

        let nodes: Vec<_> = selected_nodes
            .iter()
            .map(|id| {
                serde_json::json!({
                    "id": id,
                    "title": id_to_title.get(id).unwrap_or(id),
                })
            })
            .collect();

        let edges_json: Vec<_> = selected_edges
            .iter()
            .map(|(s, t)| serde_json::json!({ "source": s, "target": t }))
            .collect();

        Ok(serde_json::json!({
            "success": true,
            "count": nodes.len(),
            "edge_count": edges_json.len(),
            "nodes": nodes,
            "edges": edges_json,
        }))
    }

    fn export_vault(&self, output_dir: &str) -> anyhow::Result<serde_json::Value> {
        let vault_dir = self.storage.workspace_dir()?.join("vault");
        let out = std::path::PathBuf::from(output_dir);
        crate::vault::export::export_vault(&vault_dir, &out)
    }
}
