// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (c) 2026 juice094
//! devbase-symbol-links — Code symbol link generator.
//!
//! **提取日期**: 2026-05-01 (Workspace split)
//! **零内部耦合**: 此 crate 不依赖 devbase 任何内部模块，仅使用 rusqlite + chrono + anyhow。
//! **职责**: 计算代码符号间的概念关系（签名相似度 Jaccard / 同文件聚类），并持久化到 SQLite。
//! **边界**: 输入 `rusqlite::Connection` + repo_id，输出 `Vec<SymbolLink>`。调用方负责事务管理。
//!
//! 与 devbase 的关系: 被 devbase `scan` 流程调用，生成 `code_symbol_links` 表数据 (Schema v13)。
//!
//! Design decisions:
//! - Jaccard threshold 默认 0.3: 经验值，平衡召回与精度。
//! - co_located strength 固定 0.5: 同文件是中等信号，不区分文件大小。
//! - Tokenization 排除 Rust 关键字: 避免 `fn`/`pub`/`async` 等噪音影响相似度。

pub mod co_located;
pub mod similarity;

/// A generated link between two symbols.
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolLink {
    pub source_repo: String,
    pub source_symbol: String,
    pub target_repo: String,
    pub target_symbol: String,
    pub link_type: String,
    pub strength: f32,
}

/// Build all link types for a repo and persist to `code_symbol_links`.
pub fn generate_and_save_links(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<usize> {
    let mut all_links = Vec::new();
    all_links.extend(similarity::compute_similar_signature_links(conn, repo_id, 0.3)?);
    all_links.extend(co_located::compute_co_located_links(conn, repo_id)?);

    if all_links.is_empty() {
        return Ok(0);
    }

    let tx = conn.transaction()?;
    tx.execute("DELETE FROM code_symbol_links WHERE source_repo = ?1", [repo_id])?;

    let now = chrono::Utc::now().to_rfc3339();
    let mut inserted = 0;
    for link in all_links {
        tx.execute(
            "INSERT OR IGNORE INTO code_symbol_links
             (source_repo, source_symbol, target_repo, target_symbol, link_type, strength, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                &link.source_repo,
                &link.source_symbol,
                &link.target_repo,
                &link.target_symbol,
                &link.link_type,
                link.strength,
                &now,
            ],
        )?;
        inserted += 1;
    }
    tx.commit()?;
    Ok(inserted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_co_located_links() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE code_symbols (
                repo_id TEXT, file_path TEXT, symbol_type TEXT, name TEXT,
                line_start INTEGER, line_end INTEGER, signature TEXT,
                PRIMARY KEY(repo_id, file_path, name)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_symbols VALUES
             ('r1', 'src/lib.rs', 'function', 'foo', 1, 2, 'fn foo()'),
             ('r1', 'src/lib.rs', 'function', 'bar', 3, 4, 'fn bar()'),
             ('r1', 'src/main.rs', 'function', 'main', 1, 2, 'fn main()')",
            [],
        )
        .unwrap();

        let links = co_located::compute_co_located_links(&conn, "r1").unwrap();
        // lib.rs has foo+bar => 2 bidirectional links
        assert_eq!(links.len(), 2);
        // main.rs has only main => no links
        assert!(links.iter().all(|l| l.source_symbol != "main"));
    }

    #[test]
    fn test_compute_similar_signature_links() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute(
            "CREATE TABLE code_symbols (
                repo_id TEXT, file_path TEXT, symbol_type TEXT, name TEXT,
                line_start INTEGER, line_end INTEGER, signature TEXT,
                PRIMARY KEY(repo_id, file_path, name)
            )",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO code_symbols VALUES
             ('r1', 'a.rs', 'function', 'auth_token', 1, 2, 'fn auth_token(token: &str, timeout: u64)'),
             ('r1', 'a.rs', 'function', 'validate_token', 3, 4, 'fn validate_token(t: &str, timeout: u64) -> bool'),
             ('r1', 'a.rs', 'function', 'parse_config', 5, 6, 'fn parse_config(s: &str) -> Config')",
            [],
        )
        .unwrap();

        let links = similarity::compute_similar_signature_links(&conn, "r1", 0.3).unwrap();
        // auth_token and validate_token share token, str, timeout => should link
        assert!(!links.is_empty());
        let has_auth_validate = links.iter().any(|l| {
            (l.source_symbol == "auth_token" && l.target_symbol == "validate_token")
                || (l.source_symbol == "validate_token" && l.target_symbol == "auth_token")
        });
        assert!(has_auth_validate, "auth_token and validate_token should be linked");
    }
}
