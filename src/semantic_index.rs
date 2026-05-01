//! Semantic code indexing using tree-sitter.
//!
//! Extracts symbols (functions, structs, enums, traits, impls, classes,
//! interfaces) from source files and stores them in the SQLite registry for
//! AI-powered code queries.
//!
//! Supported languages: Rust (.rs), Python (.py), JavaScript/TypeScript
//! (.js, .ts, .jsx, .tsx), Go (.go).

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
    Class,
    Interface,
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
            SymbolType::Class => "class",
            SymbolType::Interface => "interface",
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

/// Row type returned by semantic search:
/// (repo_id, symbol_name, file_path, line_start, similarity_score).
pub type SemanticSearchRow = (String, String, String, i64, f32);

// ---------------------------------------------------------------------------
// Language dispatch
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
enum Lang {
    Rust,
    Python,
    JsTs,
    Go,
}

impl Lang {
    fn from_ext(ext: &str) -> Option<Self> {
        match ext {
            "rs" => Some(Lang::Rust),
            "py" => Some(Lang::Python),
            "js" | "ts" | "jsx" => Some(Lang::JsTs),
            "tsx" => Some(Lang::JsTs),
            "go" => Some(Lang::Go),
            _ => None,
        }
    }

    fn parser_language(self) -> tree_sitter::Language {
        match self {
            Lang::Rust => tree_sitter_rust::LANGUAGE.into(),
            Lang::Python => tree_sitter_python::LANGUAGE.into(),
            Lang::JsTs => tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
            Lang::Go => tree_sitter_go::LANGUAGE.into(),
        }
    }
}

// ---------------------------------------------------------------------------
// Symbol extraction
// ---------------------------------------------------------------------------

/// Extract symbols from a single source file.
///
/// Supports Rust, Python, JavaScript/TypeScript, and Go.
pub fn extract_symbols(file_path: &Path, source: &str) -> Vec<CodeSymbol> {
    let ext = file_path.extension().and_then(|e| e.to_str());
    let lang = match ext {
        Some(ext) => match Lang::from_ext(ext) {
            Some(l) => l,
            None => {
                debug!("Skipping semantic extraction for {:?}", file_path);
                return Vec::new();
            }
        },
        None => return Vec::new(),
    };

    extract_symbols_with_parser(file_path, source, lang)
}

fn extract_symbols_with_parser(file_path: &Path, source: &str, lang: Lang) -> Vec<CodeSymbol> {
    let mut parser = tree_sitter::Parser::new();
    let ts_lang = lang.parser_language();
    if let Err(e) = parser.set_language(&ts_lang) {
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

    let mut symbols = Vec::new();
    let source_bytes = source.as_bytes();
    let root = tree.root_node();
    collect_symbols_from_node(&root, file_path, source_bytes, lang, &mut symbols);
    symbols
}

fn collect_symbols_from_node(
    node: &tree_sitter::Node,
    file_path: &Path,
    source_bytes: &[u8],
    lang: Lang,
    symbols: &mut Vec<CodeSymbol>,
) {
    if let Some(sym) = node_to_symbol(node, file_path, source_bytes, lang) {
        symbols.push(sym);
        // Don't recurse into this node — we don't want inner methods of a
        // class to also be extracted as top-level symbols.
        return;
    }
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32) {
            collect_symbols_from_node(&child, file_path, source_bytes, lang, symbols);
        }
    }
}

fn node_to_symbol(
    node: &tree_sitter::Node,
    file_path: &Path,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<CodeSymbol> {
    match lang {
        Lang::Rust => rust_node_to_symbol(node, file_path, source_bytes),
        Lang::Python => python_node_to_symbol(node, file_path, source_bytes),
        Lang::JsTs => js_node_to_symbol(node, file_path, source_bytes),
        Lang::Go => go_node_to_symbol(node, file_path, source_bytes),
    }
}

// ---- Rust -----------------------------------------------------------------

fn rust_node_to_symbol(
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
    let line_start = node.start_position().row + 1;
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

// ---- Python ---------------------------------------------------------------

fn python_node_to_symbol(
    node: &tree_sitter::Node,
    file_path: &Path,
    source_bytes: &[u8],
) -> Option<CodeSymbol> {
    let symbol_type = match node.kind() {
        "function_definition" => SymbolType::Function,
        "class_definition" => SymbolType::Class,
        _ => return None,
    };

    let name = extract_node_name(node, source_bytes)?;
    let line_start = node.start_position().row + 1;
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

// ---- JavaScript / TypeScript ----------------------------------------------

fn js_node_to_symbol(
    node: &tree_sitter::Node,
    file_path: &Path,
    source_bytes: &[u8],
) -> Option<CodeSymbol> {
    let symbol_type = match node.kind() {
        "function_declaration" => SymbolType::Function,
        "method_definition" => SymbolType::Function,
        "class_declaration" => SymbolType::Class,
        "interface_declaration" => SymbolType::Interface,
        "type_alias_declaration" => SymbolType::TypeAlias,
        "enum_declaration" => SymbolType::Enum,
        _ => return None,
    };

    let name = if node.kind() == "method_definition" {
        extract_child_by_kind(node, "property_identifier", source_bytes)?
    } else {
        extract_node_name(node, source_bytes)?
    };

    let line_start = node.start_position().row + 1;
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

// ---- Go -------------------------------------------------------------------

fn go_node_to_symbol(
    node: &tree_sitter::Node,
    file_path: &Path,
    source_bytes: &[u8],
) -> Option<CodeSymbol> {
    match node.kind() {
        "function_declaration" => {
            let name = extract_node_name(node, source_bytes)?;
            let line_start = node.start_position().row + 1;
            let line_end = node.end_position().row + 1;
            let signature = extract_signature(node, source_bytes);
            Some(CodeSymbol {
                symbol_type: SymbolType::Function,
                name,
                file_path: file_path.to_path_buf(),
                line_start,
                line_end,
                signature,
            })
        }
        "method_declaration" => {
            let name = extract_child_by_kind(node, "field_identifier", source_bytes)?;
            let line_start = node.start_position().row + 1;
            let line_end = node.end_position().row + 1;
            let signature = extract_signature(node, source_bytes);
            Some(CodeSymbol {
                symbol_type: SymbolType::Function,
                name,
                file_path: file_path.to_path_buf(),
                line_start,
                line_end,
                signature,
            })
        }
        "type_spec" => {
            let name = extract_child_by_kind(node, "type_identifier", source_bytes)?;
            let symbol_type = if has_child_of_kind(node, "struct_type") {
                SymbolType::Struct
            } else if has_child_of_kind(node, "interface_type") {
                SymbolType::Interface
            } else {
                SymbolType::TypeAlias
            };
            let line_start = node.start_position().row + 1;
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
        "const_spec" => {
            let name = extract_child_by_kind(node, "identifier", source_bytes)?;
            let line_start = node.start_position().row + 1;
            let line_end = node.end_position().row + 1;
            let signature = extract_signature(node, source_bytes);
            Some(CodeSymbol {
                symbol_type: SymbolType::Constant,
                name,
                file_path: file_path.to_path_buf(),
                line_start,
                line_end,
                signature,
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_node_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i as u32)?;
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return Some(node_text(&child, source_bytes).to_string());
        }
    }
    None
}

fn extract_child_by_kind(
    node: &tree_sitter::Node,
    kind: &str,
    source_bytes: &[u8],
) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i as u32)?;
        if child.kind() == kind {
            return Some(node_text(&child, source_bytes).to_string());
        }
    }
    None
}

fn has_child_of_kind(node: &tree_sitter::Node, kind: &str) -> bool {
    for i in 0..node.child_count() {
        if let Some(child) = node.child(i as u32)
            && child.kind() == kind
        {
            return true;
        }
    }
    false
}

fn extract_signature(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    let block_kinds = [
        "block",                  // Rust, Python, Go
        "statement_block",        // JS/TS functions
        "class_body",             // JS/TS classes
        "enum_body",              // JS/TS enums
        "interface_body",         // TS interfaces
        "field_declaration_list", // Go structs
        "method_spec_list",       // Go interfaces
    ];
    for i in 0..node.child_count() {
        let child = node.child(i as u32)?;
        if block_kinds.contains(&child.kind()) {
            let sig_text = &source_bytes[node.start_byte()..child.start_byte()];
            return Some(String::from_utf8_lossy(sig_text).trim().to_string());
        }
    }
    // Fallback: first line
    let line_start = node.start_position().row;
    let line_text = source_bytes.split(|&b| b == b'\n').nth(line_start)?;
    Some(String::from_utf8_lossy(line_text).trim().to_string())
}

fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Repository-wide indexing
// ---------------------------------------------------------------------------

/// Scan a repository for source files and extract all symbols and call relationships.
pub fn index_repo_full(repo_path: &Path) -> (Vec<CodeSymbol>, Vec<CodeCall>) {
    let exts: &[&str] = &["rs", "py", "js", "ts", "jsx", "tsx", "go"];

    let files: Vec<std::path::PathBuf> = walkdir::WalkDir::new(repo_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let ext = e.path().extension().and_then(|e| e.to_str());
            ext.is_some_and(|e| exts.contains(&e))
        })
        .map(|e| e.path().to_path_buf())
        .collect();

    let num_threads = std::env::var("DEVBASE_INDEX_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| {
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1)
                .min(8)
        });

    if num_threads <= 1 || files.len() < 16 {
        let mut all_symbols = Vec::new();
        let mut all_calls = Vec::new();
        for path in files {
            process_file(repo_path, &path, &mut all_symbols, &mut all_calls);
        }
        return (all_symbols, all_calls);
    }

    std::thread::scope(|s| {
        let chunk_size = (files.len() + num_threads - 1) / num_threads;
        let mut handles = Vec::with_capacity(num_threads);

        for chunk in files.chunks(chunk_size) {
            handles.push(
                std::thread::Builder::new()
                    .stack_size(4 * 1024 * 1024)
                    .spawn_scoped(s, move || {
                        let mut symbols = Vec::new();
                        let mut calls = Vec::new();
                        for path in chunk {
                            process_file(repo_path, path, &mut symbols, &mut calls);
                        }
                        (symbols, calls)
                    })
                    .expect("failed to spawn index worker"),
            );
        }

        let mut all_symbols = Vec::new();
        let mut all_calls = Vec::new();
        for handle in handles {
            let (s, c) = handle.join().unwrap();
            all_symbols.extend(s);
            all_calls.extend(c);
        }
        (all_symbols, all_calls)
    })
}

#[inline]
fn process_file(
    repo_path: &Path,
    path: &std::path::PathBuf,
    all_symbols: &mut Vec<CodeSymbol>,
    all_calls: &mut Vec<CodeCall>,
) {
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

/// Scan a repository for source files and extract all symbols.
pub fn index_repo(repo_path: &Path) -> Vec<CodeSymbol> {
    index_repo_full(repo_path).0
}

// ---------------------------------------------------------------------------
// Call extraction
// ---------------------------------------------------------------------------

fn extract_calls_from_file(file_path: &Path, source: &str) -> Vec<CodeCall> {
    let ext = file_path.extension().and_then(|e| e.to_str());
    let lang = match ext {
        Some(ext) => match Lang::from_ext(ext) {
            Some(l) => l,
            None => return Vec::new(),
        },
        None => return Vec::new(),
    };

    let mut parser = tree_sitter::Parser::new();
    let ts_lang = lang.parser_language();
    if let Err(e) = parser.set_language(&ts_lang) {
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
    walk_tree_for_calls(&mut cursor, file_path, source_bytes, lang, &mut calls, None);
    calls
}

fn walk_tree_for_calls(
    cursor: &mut tree_sitter::TreeCursor,
    file_path: &Path,
    source_bytes: &[u8],
    lang: Lang,
    calls: &mut Vec<CodeCall>,
    current_function: Option<&str>,
) {
    let node = cursor.node();

    let func_name = extract_current_function_name(&node, source_bytes, lang);
    let func_name_ref = func_name.as_deref().or(current_function);

    if let Some(callee) = extract_callee_name(&node, source_bytes, lang)
        && let Some(caller) = func_name_ref
    {
        calls.push(CodeCall {
            caller_file: file_path.to_path_buf(),
            caller_symbol: caller.to_string(),
            caller_line: node.start_position().row + 1,
            callee_name: callee,
        });
    }

    if cursor.goto_first_child() {
        loop {
            walk_tree_for_calls(cursor, file_path, source_bytes, lang, calls, func_name_ref);
            if !cursor.goto_next_sibling() {
                break;
            }
        }
        cursor.goto_parent();
    }
}

fn extract_current_function_name(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<String> {
    match lang {
        Lang::Rust => {
            if node.kind() == "function_item" || node.kind() == "closure_expression" {
                extract_node_name(node, source_bytes)
            } else {
                None
            }
        }
        Lang::Python => {
            if node.kind() == "function_definition" {
                extract_node_name(node, source_bytes)
            } else {
                None
            }
        }
        Lang::JsTs => {
            if node.kind() == "function_declaration" || node.kind() == "method_definition" {
                if node.kind() == "method_definition" {
                    extract_child_by_kind(node, "property_identifier", source_bytes)
                } else {
                    extract_node_name(node, source_bytes)
                }
            } else {
                None
            }
        }
        Lang::Go => {
            if node.kind() == "function_declaration" {
                extract_node_name(node, source_bytes)
            } else if node.kind() == "method_declaration" {
                extract_child_by_kind(node, "field_identifier", source_bytes)
            } else {
                None
            }
        }
    }
}

fn extract_callee_name(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
    lang: Lang,
) -> Option<String> {
    match lang {
        Lang::Rust => {
            if node.kind() == "call_expression" {
                let func_node = node.child(0)?;
                extract_rust_call_target_name(&func_node, source_bytes)
            } else if node.kind() == "macro_invocation" {
                extract_macro_name(node, source_bytes)
            } else {
                None
            }
        }
        Lang::Python => {
            if node.kind() == "call" {
                let func_node = node.child(0)?;
                extract_python_call_target_name(&func_node, source_bytes)
            } else {
                None
            }
        }
        Lang::JsTs => {
            if node.kind() == "call_expression" {
                let func_node = node.child(0)?;
                extract_js_call_target_name(&func_node, source_bytes)
            } else {
                None
            }
        }
        Lang::Go => {
            if node.kind() == "call_expression" {
                let func_node = node.child(0)?;
                extract_go_call_target_name(&func_node, source_bytes)
            } else {
                None
            }
        }
    }
}

fn extract_rust_call_target_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source_bytes).to_string()),
        "field_expression" => {
            for i in 0..node.child_count() {
                let child = node.child(i as u32)?;
                if child.kind() == "field_identifier" {
                    return Some(node_text(&child, source_bytes).to_string());
                }
            }
            None
        }
        "scoped_identifier" => {
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

fn extract_python_call_target_name(
    node: &tree_sitter::Node,
    source_bytes: &[u8],
) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source_bytes).to_string()),
        "attribute" => {
            // obj.method  — take the attribute (method name)
            for i in 0..node.child_count() {
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

fn extract_js_call_target_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source_bytes).to_string()),
        "member_expression" => {
            for i in 0..node.child_count() {
                let child = node.child(i as u32)?;
                if child.kind() == "property_identifier" {
                    return Some(node_text(&child, source_bytes).to_string());
                }
            }
            None
        }
        _ => None,
    }
}

fn extract_go_call_target_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    match node.kind() {
        "identifier" => Some(node_text(node, source_bytes).to_string()),
        "selector_expression" => {
            for i in 0..node.child_count() {
                let child = node.child(i as u32)?;
                if child.kind() == "field_identifier" {
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

// ---------------------------------------------------------------------------
// Batch save to SQLite
// ---------------------------------------------------------------------------

/// Batch save symbols to the SQLite registry.
pub fn save_symbols(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    symbols: &[CodeSymbol],
) -> anyhow::Result<usize> {
    let tx = conn.transaction()?;

    // Clear old symbols for this repo
    tx.execute("DELETE FROM code_symbols WHERE repo_id = ?1", [repo_id])?;

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
    tx.execute("DELETE FROM code_call_graph WHERE repo_id = ?1", [repo_id])?;

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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Rust ----------------------------------------------------------------

    #[test]
    fn test_extract_rust_function() {
        let source = r#"
fn hello_world() -> String {
    "hello".to_string()
}
"#;
        let symbols = extract_symbols(Path::new("test.rs"), source);
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
        let symbols = extract_symbols(Path::new("test.rs"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Struct);
        assert_eq!(symbols[0].name, "RepoEntry");
    }

    #[test]
    fn test_extract_multiple_rust() {
        let source = r#"
fn func_a() {}
fn func_b() {}
struct MyStruct;
"#;
        let symbols = extract_symbols(Path::new("test.rs"), source);
        assert_eq!(symbols.len(), 3);
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"func_a"));
        assert!(names.contains(&"func_b"));
        assert!(names.contains(&"MyStruct"));
    }

    // ---- Python --------------------------------------------------------------

    #[test]
    fn test_extract_python_function() {
        let source = r#"
def hello_world():
    return "hello"
"#;
        let symbols = extract_symbols(Path::new("test.py"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Function);
        assert_eq!(symbols[0].name, "hello_world");
        assert_eq!(symbols[0].line_start, 2);
        assert!(symbols[0].signature.is_some());
    }

    #[test]
    fn test_extract_python_class() {
        let source = r#"
class MyClass:
    def method(self):
        pass
"#;
        let symbols = extract_symbols(Path::new("test.py"), source);
        // Only the class is extracted at top-level; inner method is skipped.
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Class);
        assert_eq!(symbols[0].name, "MyClass");
    }

    #[test]
    fn test_extract_python_multiple() {
        let source = r#"
def func_a():
    pass

def func_b():
    pass

class MyClass:
    pass
"#;
        let symbols = extract_symbols(Path::new("test.py"), source);
        assert_eq!(symbols.len(), 3);
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"func_a"));
        assert!(names.contains(&"func_b"));
        assert!(names.contains(&"MyClass"));
    }

    // ---- JavaScript / TypeScript ---------------------------------------------

    #[test]
    fn test_extract_js_function() {
        let source = r#"
function helloWorld() {
    return "hello";
}
"#;
        let symbols = extract_symbols(Path::new("test.js"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Function);
        assert_eq!(symbols[0].name, "helloWorld");
        assert_eq!(symbols[0].line_start, 2);
        assert!(symbols[0].signature.is_some());
    }

    #[test]
    fn test_extract_ts_class_and_interface() {
        let source = r#"
interface Point {
    x: number;
    y: number;
}

class MyClass implements Point {
    x: number = 0;
    y: number = 0;
}
"#;
        let symbols = extract_symbols(Path::new("test.ts"), source);
        assert_eq!(symbols.len(), 2);
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Point"));
        assert!(names.contains(&"MyClass"));
        let types: Vec<_> = symbols.iter().map(|s| s.symbol_type.clone()).collect();
        assert!(types.contains(&SymbolType::Interface));
        assert!(types.contains(&SymbolType::Class));
    }

    #[test]
    fn test_extract_ts_enum() {
        let source = r#"
enum Color {
    Red,
    Green,
    Blue,
}
"#;
        let symbols = extract_symbols(Path::new("test.ts"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Enum);
        assert_eq!(symbols[0].name, "Color");
    }

    // ---- Go ------------------------------------------------------------------

    #[test]
    fn test_extract_go_function() {
        let source = r#"
package main

func helloWorld() string {
    return "hello"
}
"#;
        let symbols = extract_symbols(Path::new("test.go"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Function);
        assert_eq!(symbols[0].name, "helloWorld");
        assert_eq!(symbols[0].line_start, 4);
        assert!(symbols[0].signature.is_some());
    }

    #[test]
    fn test_extract_go_struct_and_interface() {
        let source = r#"
package main

type Reader interface {
    Read(p []byte) (n int, err error)
}

type MyStruct struct {
    Name string
}
"#;
        let symbols = extract_symbols(Path::new("test.go"), source);
        assert_eq!(symbols.len(), 2);
        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"Reader"));
        assert!(names.contains(&"MyStruct"));
        let types: Vec<_> = symbols.iter().map(|s| s.symbol_type.clone()).collect();
        assert!(types.contains(&SymbolType::Interface));
        assert!(types.contains(&SymbolType::Struct));
    }

    #[test]
    fn test_extract_go_const() {
        let source = r#"
package main

const Pi = 3.14
"#;
        let symbols = extract_symbols(Path::new("test.go"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Constant);
        assert_eq!(symbols[0].name, "Pi");
    }

    #[test]
    fn test_extract_go_method() {
        let source = r#"
package main

func (s *MyStruct) Method() string {
    return s.Name
}
"#;
        let symbols = extract_symbols(Path::new("test.go"), source);
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].symbol_type, SymbolType::Function);
        assert_eq!(symbols[0].name, "Method");
    }

    #[test]
    fn test_save_symbols() {
        let mut conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let symbols = vec![
            CodeSymbol {
                symbol_type: SymbolType::Function,
                name: "hello".to_string(),
                file_path: std::path::PathBuf::from("src/main.rs"),
                line_start: 1,
                line_end: 3,
                signature: Some("fn hello()".to_string()),
            },
            CodeSymbol {
                symbol_type: SymbolType::Struct,
                name: "Point".to_string(),
                file_path: std::path::PathBuf::from("src/lib.rs"),
                line_start: 5,
                line_end: 8,
                signature: None,
            },
        ];

        let count = save_symbols(&mut conn, "repo-a", &symbols).unwrap();
        assert_eq!(count, 2);

        let mut stmt = conn
            .prepare("SELECT name, symbol_type FROM code_symbols WHERE repo_id = ?1 ORDER BY name")
            .unwrap();
        let rows = stmt
            .query_map(["repo-a"], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .unwrap();
        let results: Vec<_> = rows.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, "Point");
        assert_eq!(results[0].1, "struct");
        assert_eq!(results[1].0, "hello");
        assert_eq!(results[1].1, "function");
    }

    #[test]
    fn test_save_calls() {
        let mut conn = crate::registry::WorkspaceRegistry::init_in_memory().unwrap();
        let calls = vec![CodeCall {
            caller_file: std::path::PathBuf::from("src/main.rs"),
            caller_symbol: "main".to_string(),
            caller_line: 10,
            callee_name: "helper".to_string(),
        }];

        let count = save_calls(&mut conn, "repo-a", &calls).unwrap();
        assert_eq!(count, 1);

        let mut stmt = conn
            .prepare("SELECT caller_symbol, callee_name FROM code_call_graph WHERE repo_id = ?1")
            .unwrap();
        let rows = stmt
            .query_map(["repo-a"], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .unwrap();
        let results: Vec<_> = rows.collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "main");
        assert_eq!(results[0].1, "helper");
    }

    #[test]
    fn test_index_repo_full() {
        let tmp = tempfile::tempdir().unwrap();
        let src_dir = tmp.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();

        std::fs::write(
            src_dir.join("main.rs"),
            r#"
fn main() {
    hello();
}

fn hello() -> &'static str {
    "hello"
}

struct Point { x: i32, y: i32 }
"#,
        )
        .unwrap();

        std::fs::write(
            src_dir.join("lib.py"),
            r#"
def helper():
    return 42

class MyClass:
    pass
"#,
        )
        .unwrap();

        let (symbols, calls) = index_repo_full(tmp.path());
        assert!(!symbols.is_empty());

        let names: Vec<_> = symbols.iter().map(|s| s.name.as_str()).collect();
        assert!(names.contains(&"main"));
        assert!(names.contains(&"hello"));
        assert!(names.contains(&"Point"));
        assert!(names.contains(&"helper"));
        assert!(names.contains(&"MyClass"));

        // Calls: main -> hello
        assert!(!calls.is_empty());
        let call_names: Vec<_> = calls.iter().map(|c| c.callee_name.as_str()).collect();
        assert!(call_names.contains(&"hello"));
    }
}
