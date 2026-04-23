//! Semantic code indexing using tree-sitter.
//!
//! Extracts symbols (functions, structs, enums, traits, impls) from source files
//! and stores them in the SQLite registry for AI-powered code queries.

use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// A extracted code symbol.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeSymbol {
    pub symbol_type: SymbolType,
    pub name: String,
    pub file_path: PathBuf,
    pub line_start: usize,
    pub line_end: usize,
    pub signature: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SymbolType {
    Function,
    Struct,
    Enum,
    Trait,
    Impl,
    Module,
    TypeAlias,
    Constant,
    Static,
}

impl SymbolType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SymbolType::Function => "function",
            SymbolType::Struct => "struct",
            SymbolType::Enum => "enum",
            SymbolType::Trait => "trait",
            SymbolType::Impl => "impl",
            SymbolType::Module => "module",
            SymbolType::TypeAlias => "type_alias",
            SymbolType::Constant => "constant",
            SymbolType::Static => "static",
        }
    }
}

/// A call relationship: caller_symbol calls callee_name at caller_line.
#[derive(Debug, Clone, PartialEq)]
pub struct CodeCall {
    pub caller_file: PathBuf,
    pub caller_symbol: String,
    pub caller_line: usize,
    pub callee_name: String,
}

/// Extract symbols from a single source file.
///
/// Currently supports Rust. Other languages return empty Vec.
pub fn extract_symbols(file_path: &Path, source: &str) -> Vec<CodeSymbol> {
    let ext = file_path.extension().and_then(|e| e.to_str());
    match ext {
        Some("rs") => extract_rust_symbols(file_path, source),
        _ => {
            debug!("Skipping semantic extraction for {:?}", file_path);
            Vec::new()
        }
    }
}

/// Extract Rust symbols using tree-sitter.
fn extract_rust_symbols(file_path: &Path, source: &str) -> Vec<CodeSymbol> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    if let Err(e) = parser.set_language(&language) {
        warn!("Failed to set tree-sitter language: {}", e);
        return Vec::new();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            warn!("Failed to parse {:?}", file_path);
            return Vec::new();
        }
    };

    let root = tree.root_node();
    let mut symbols = Vec::new();
    let source_bytes = source.as_bytes();

    for i in 0..root.child_count() {
        let node = root.child(i as u32).unwrap();
        if let Some(sym) = node_to_symbol(&node, file_path, source_bytes) {
            symbols.push(sym);
        }
    }

    symbols
}

fn node_to_symbol(
    node: &tree_sitter::Node,
    file_path: &Path,
    source_bytes: &[u8],
) -> Option<CodeSymbol> {
    let symbol_type = match node.kind() {
        "function_item" => SymbolType::Function,
        "struct_item" => SymbolType::Struct,
        "enum_item" => SymbolType::Enum,
        "trait_item" => SymbolType::Trait,
        "impl_item" => SymbolType::Impl,
        "module" => SymbolType::Module,
        "type_item" => SymbolType::TypeAlias,
        "const_item" => SymbolType::Constant,
        "static_item" => SymbolType::Static,
        _ => return None,
    };

    let name = extract_node_name(node, source_bytes)?;
    let line_start = node.start_position().row + 1; // 1-based
    let line_end = node.end_position().row + 1;
    let signature = extract_signature(node, source_bytes);

    Some(CodeSymbol {
        symbol_type,
        name,
        file_path: file_path.to_path_buf(),
        line_start,
        line_end,
        signature,
    })
}

fn extract_node_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    // Find the identifier or type_identifier child
    for i in 0..node.child_count() {
        let child = node.child(i as u32)?;
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return Some(node_text(&child, source_bytes).to_string());
        }
    }
    None
}

fn extract_signature(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    // For functions, extract the full signature line (fn name(...))
    if node.kind() == "function_item" {
        let start = node.start_position();
        let _end_byte = node.end_byte();
        // Find the end of the function signature (before the body block)
        for i in 0..node.child_count() {
            let child = node.child(i as u32)?;
            if child.kind() == "block" {
                let sig_text = &source_bytes[node.start_byte()..child.start_byte()];
                return Some(String::from_utf8_lossy(sig_text).trim().to_string());
            }
        }
        // Fallback: first line
        let line_start = start.row;
        let line_text = source_bytes
            .split(|&b| b == b'\n')
            .nth(line_start)?;
        return Some(String::from_utf8_lossy(line_text).trim().to_string());
    }
    None
}

fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

/// Scan a repository for source files and extract all symbols and call relationships.
pub fn index_repo_full(repo_path: &Path) -> (Vec<CodeSymbol>, Vec<CodeCall>) {
    let mut all_symbols = Vec::new();
    let mut all_calls = Vec::new();

    for entry in walkdir::WalkDir::new(repo_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        let path = entry.path();
        if path.extension() != Some("rs".as_ref()) {
            continue;
        }

        match std::fs::read_to_string(path) {
            Ok(source) => {
                let rel_path = path.strip_prefix(repo_path).unwrap_or(path);
                let symbols = extract_symbols(rel_path, &source);
                all_symbols.extend(symbols);

                let calls = extract_calls_from_file(rel_path, &source);
                all_calls.extend(calls);
            }
            Err(e) => {
                warn!("Failed to read {:?}: {}", path, e);
            }
        }
    }

    (all_symbols, all_calls)
}

/// Scan a repository for source files and extract all symbols.
pub fn index_repo(repo_path: &Path) -> Vec<CodeSymbol> {
    index_repo_full(repo_path).0
}

fn extract_calls_from_file(file_path: &Path, source: &str) -> Vec<CodeCall> {
    let mut parser = tree_sitter::Parser::new();
    let language = tree_sitter_rust::LANGUAGE.into();
    if let Err(e) = parser.set_language(&language) {
        warn!("Failed to set tree-sitter language: {}", e);
        return Vec::new();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            warn!("Failed to parse {:?}", file_path);
            return Vec::new();
        }
    };

    let mut calls = Vec::new();
    let source_bytes = source.as_bytes();
    let root = tree.root_node();
    let mut cursor = root.walk();
    walk_tree_for_calls(&mut cursor, file_path, source_bytes, &mut calls, None);
    calls
}

fn walk_tree_for_calls(
    cursor: &mut tree_sitter::TreeCursor,
    file_path: &Path,
    source_bytes: &[u8],
    calls: &mut Vec<CodeCall>,
    current_function: Option<&str>,
) {
    let node = cursor.node();

    let func_name: Option<String> = if node.kind() == "function_item" || node.kind() == "closure_expression" {
        extract_node_name(&node, source_bytes)
    } else {
        None
    };
    let func_name_ref = func_name.as_deref().or(current_function);

    if node.kind() == "call_expression" {
        if let Some(callee) = extract_callee_name(&node, source_bytes) {
            if let Some(caller) = func_name_ref {
                calls.push(CodeCall {
                    caller_file: file_path.to_path_buf(),
                    caller_symbol: caller.to_string(),
                    caller_line: node.start_position().row + 1,
                    callee_name: callee,
                });
            }
        }
    }

    if node.kind() == "macro_invocation" {
        if let Some(callee) = extract_macro_name(&node, source_bytes) {
            if let Some(caller) = func_name_ref {
                calls.push(CodeCall {
                    caller_file: file_path.to_path_buf(),
                    caller_symbol: caller.to_string(),
                    caller_line: node.start_position().row + 1,
                    callee_name: callee,
                });
            }
        }
    }

    if cursor.goto_first_child() {
        loop {
            walk_tree_for_calls(cursor, file_path, source_bytes, calls, func_name_ref);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn extract_callee_name(call_node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    // call_expression children: function expression, arguments
    let func_node = call_node.child(0)?;
    extract_call_target_name(&func_node, source_bytes)
}

fn extract_call_target_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source_bytes).to_string()),
        "field_expression" => {
            // Method call: self.foo() or obj.bar()
            for i in 0..node.child_count() {
                let child = node.child(i as u32)?;
                if child.kind() == "field_identifier" {
                    return Some(node_text(&child, source_bytes).to_string());
                }
            }
            None
        }
        "scoped_identifier" => {
            // Foo::bar() or crate::foo::bar()
            for i in (0..node.child_count()).rev() {
                let child = node.child(i as u32)?;
                if child.kind() == "identifier" {
                    return Some(node_text(&child, source_bytes).to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_macro_name(macro_node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    for i in 0..macro_node.child_count() {
        let child = macro_node.child(i as u32)?;
        if child.kind() == "identifier" {
            return Some(node_text(&child, source_bytes).to_string());
        }
    }
    None
}

/// Batch save symbols to the SQLite registry.
pub fn save_symbols(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[CodeSymbol],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;

    // Clear old symbols for this repo
    tx.execute(
        "DELETE FROM code_symbols WHERE repo_id = ?1",
        [repo_id],
    )?;

    let mut inserted = 0;
    for sym in symbols {
        tx.execute(
            "INSERT INTO code_symbols
             (repo_id, file_path, symbol_type, name, line_start, line_end, signature)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(repo_id, file_path, name) DO UPDATE SET
             symbol_type = excluded.symbol_type,
             line_start = excluded.line_start,
             line_end = excluded.line_end,
             signature = excluded.signature",
            (
                repo_id,
                sym.file_path.to_string_lossy().as_ref(),
                sym.symbol_type.as_str(),
                &sym.name,
                sym.line_start as i64,
                sym.line_end as i64,
                sym.signature.as_deref(),
            ),
        )?;
        inserted += 1;
    }

    tx.commit()?;
    Ok(inserted)
}

/// Batch save call relationships to the SQLite registry.
pub fn save_calls(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    calls: &[CodeCall],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;

    // Clear old calls for this repo
    tx.execute(
        "DELETE FROM code_call_graph WHERE repo_id = ?1",
        [repo_id],
    )?;

    let mut inserted = 0;
    for call in calls {
        tx.execute(
            "INSERT INTO code_call_graph
             (repo_id, caller_file, caller_symbol, caller_line, callee_name)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT DO NOTHING",
            (
                repo_id,
                call.caller_file.to_string_lossy().as_ref(),
                &call.caller_symbol,
                call.caller_line as i64,
                &call.callee_name,
            ),
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
    fn test_extract_rust_function() {
        let source = r#"
fn hello_world() -> String {
    "hello".to_string()
}
"#;
        let symbols = extract_rust_symbols(Path::new("test.rs"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Function);
        assert_eq!(symbols[0].name, "hello_world");
        assert_eq!(symbols[0].line_start, 2);
        assert!(symbols[0].signature.is_some());
    }

    #[test]
    fn test_extract_rust_struct() {
        let source = r#"
pub struct RepoEntry {
    id: String,
    path: PathBuf,
}
"#;
        let symbols = extract_rust_symbols(Path::new("test.rs"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Struct);
        assert_eq!(symbols[0].name, "RepoEntry");
    }

    #[test]
    fn test_extract_multiple() {
        let source = r#"
fn func_a() {}
fn func_b() {}
struct MyStruct;
"#;
        let symbols = extract_rust_symbols(Path::new("test.rs"), source);
        assert_eq!(symbols.len(), 3);
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"func_a"));
        assert!(names.contains(&"func_b"));
        assert!(names.contains(&"MyStruct"));
    }
}
