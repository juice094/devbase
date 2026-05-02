//! Semantic code indexing using tree-sitter.
//!
//! Extracts symbols (functions, structs, enums, traits, impls, classes,
//! interfaces) from source files and stores them in the SQLite registry for
//! AI-powered code queries.
//!
//! Supported languages: Rust (.rs), Python (.py), JavaScript/TypeScript
//! (.js, .ts, .jsx, .tsx), Go (.go).

use std::path::{Path, PathBuf};
use tracing::warn;

pub mod call_graph;
pub mod persist;
pub mod symbol;

pub use call_graph::*;
pub use persist::*;
pub use symbol::*;

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

/// Check whether any path component matches one of the excluded directory names.
pub fn should_skip_dir(path: &Path, exclude: &[String]) -> bool {
    path.components().any(|c| {
        if let Some(name) = c.as_os_str().to_str() {
            exclude.iter().any(|ex| ex == name)
        } else {
            false
        }
    })
}

/// Extract symbols from a single source file.
///
/// Supports Rust, Python, JavaScript/TypeScript, and Go.
pub fn index_repo_full(repo_path: &Path) -> (Vec<CodeSymbol>, Vec<CodeCall>) {
    let exts: &[&str] = &["rs", "py", "js", "ts", "jsx", "tsx", "go"];
    let exclude = crate::config::default_exclude_patterns();

    let files: Vec<std::path::PathBuf> = walkdir::WalkDir::new(repo_path)
        .into_iter()
        .filter_entry(|e| !should_skip_dir(e.path(), &exclude))
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
