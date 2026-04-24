use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use devbase::embedding::cosine_similarity;
use devbase::semantic_index::{extract_symbols, index_repo_full};
use std::path::PathBuf;

fn bench_index_repo_full(c: &mut Criterion) {
    let mut group = c.benchmark_group("index_repo_full");
    group.sample_size(50);

    let src_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let rs_files: Vec<PathBuf> = walkdir::WalkDir::new(&src_dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "rs"))
        .map(|e| e.path().to_path_buf())
        .collect();

    let scales = [("small", 5usize), ("medium", 20usize), ("full", usize::MAX)];
    for (label, count) in &scales {
        let selected: Vec<_> = if *count == usize::MAX {
            rs_files.clone()
        } else {
            rs_files.iter().take(*count).cloned().collect()
        };

        let temp_dir = tempfile::tempdir().unwrap();
        for f in &selected {
            let rel = f.strip_prefix(&src_dir).unwrap_or(f);
            let dest = temp_dir.path().join(rel);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).unwrap();
            }
            std::fs::copy(f, &dest).unwrap();
        }
        let path = temp_dir.path().to_path_buf();

        group.bench_with_input(BenchmarkId::new("scale", label), &path, |b, p| {
            b.iter(|| {
                let result = index_repo_full(p);
                black_box(result);
            });
        });
    }

    group.finish();
}

fn bench_cosine_similarity(c: &mut Criterion) {
    let mut group = c.benchmark_group("cosine_similarity");
    group.sample_size(50);

    let dims = [128usize, 512usize, 768usize];
    for &dim in &dims {
        let a: Vec<f32> = (0..dim).map(|i| (i as f32).sin()).collect();
        let b: Vec<f32> = (0..dim).map(|i| (i as f32).cos()).collect();

        group.bench_with_input(BenchmarkId::new("dim", dim), &(a, b), |bencher, (va, vb)| {
            bencher.iter(|| {
                let sim = cosine_similarity(va, vb);
                black_box(sim);
            });
        });
    }

    group.finish();
}

const RUST_SNIPPET: &str = r#"
use std::collections::HashMap;

pub struct SymbolTable {
    entries: HashMap<String, u64>,
    counter: u64,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            counter: 0,
        }
    }

    pub fn insert(&mut self, name: String) -> u64 {
        let id = self.counter;
        self.counter += 1;
        self.entries.insert(name, id);
        id
    }

    pub fn lookup(&self, name: &str) -> Option<u64> {
        self.entries.get(name).copied()
    }

    pub fn merge(&mut self, other: SymbolTable) {
        for (k, v) in other.entries {
            self.entries.entry(k).or_insert(v);
        }
    }
}

fn helper_compute(x: i32, y: i32) -> i32 {
    x * y + x - y
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert() {
        let mut st = SymbolTable::new();
        assert_eq!(st.insert("a".to_string()), 0);
        assert_eq!(st.insert("b".to_string()), 1);
    }
}
"#;

const PYTHON_SNIPPET: &str = r#"
import hashlib
from typing import Dict, List, Optional

class DocumentIndex:
    def __init__(self):
        self.docs: Dict[str, str] = {}
        self.index: Dict[str, List[str]] = {}

    def add(self, doc_id: str, content: str) -> None:
        self.docs[doc_id] = content
        for word in content.split():
            self.index.setdefault(word, []).append(doc_id)

    def search(self, query: str) -> List[str]:
        results = set()
        for word in query.split():
            for doc_id in self.index.get(word, []):
                results.add(doc_id)
        return list(results)

    def delete(self, doc_id: str) -> bool:
        if doc_id not in self.docs:
            return False
        content = self.docs.pop(doc_id)
        for word in content.split():
            self.index[word].remove(doc_id)
        return True

def compute_checksum(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()

class IndexError(Exception):
    pass
"#;

const GO_SNIPPET: &str = r#"
package parser

import (
	"fmt"
	"strings"
)

type Token struct {
	Type  TokenType
	Value string
	Line  int
	Col   int
}

type TokenType int

const (
	TokenIdent TokenType = iota
	TokenNumber
	TokenString
	TokenEOF
)

type Lexer struct {
	input string
	pos   int
	line  int
	col   int
}

func NewLexer(input string) *Lexer {
	return &Lexer{input: input, line: 1, col: 1}
}

func (l *Lexer) NextToken() (Token, error) {
	if l.pos >= len(l.input) {
		return Token{Type: TokenEOF}, nil
	}
	ch := l.input[l.pos]
	if isLetter(ch) {
		return l.readIdent()
	}
	if isDigit(ch) {
		return l.readNumber()
	}
	return Token{}, fmt.Errorf("unexpected char %q at %d:%d", ch, l.line, l.col)
}

func (l *Lexer) readIdent() (Token, error) {
	start := l.pos
	for l.pos < len(l.input) && isLetter(l.input[l.pos]) {
		l.pos++
	}
	val := l.input[start:l.pos]
	return Token{Type: TokenIdent, Value: val, Line: l.line}, nil
}

func isLetter(ch byte) bool {
	return (ch >= 'a' && ch <= 'z') || (ch >= 'A' && ch <= 'Z')
}

func isDigit(ch byte) bool {
	return ch >= '0' && ch <= '9'
}
"#;

fn bench_extract_symbols(c: &mut Criterion) {
    let mut group = c.benchmark_group("extract_symbols");
    group.sample_size(50);

    let cases = [
        ("rust", PathBuf::from("bench.rs"), RUST_SNIPPET),
        ("python", PathBuf::from("bench.py"), PYTHON_SNIPPET),
        ("go", PathBuf::from("bench.go"), GO_SNIPPET),
    ];

    for (lang, path, source) in &cases {
        group.bench_with_input(
            BenchmarkId::new("lang", lang),
            &(path, source),
            |b, (path, source)| {
                b.iter(|| {
                    let symbols = extract_symbols(path, source);
                    black_box(symbols);
                });
            },
        );
    }

    group.finish();
}

const CMAKE_CONTENT: &str = r#"
cmake_minimum_required(VERSION 3.20)
project(MyProject VERSION 1.0.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# External packages
find_package(Threads REQUIRED)
find_package(ZLIB REQUIRED)
find_package(OpenSSL REQUIRED)
find_package(Boost 1.80 COMPONENTS system filesystem REQUIRED)

# Subdirectories
add_subdirectory(src)
add_subdirectory(tests)
add_subdirectory(third_party/googletest)

# FetchContent
include(FetchContent)
FetchContent_Declare(
    fmt
    GIT_REPOSITORY https://github.com/fmtlib/fmt.git
    GIT_TAG 10.1.1
)
FetchContent_Declare(
    spdlog
    GIT_REPOSITORY https://github.com/gabime/spdlog.git
    GIT_TAG v1.12.0
)
FetchContent_Declare(
    nlohmann_json
    GIT_REPOSITORY https://github.com/nlohmann/json.git
    GIT_TAG v3.11.2
)

FetchContent_MakeAvailable(fmt spdlog nlohmann_json)

# Targets
add_library(mylib STATIC src/mylib.cpp src/utils.cpp)
target_include_directories(mylib PUBLIC include)
target_link_libraries(mylib
    PRIVATE
        fmt::fmt
        spdlog::spdlog
    PUBLIC
        Threads::Threads
        ZLIB::ZLIB
        OpenSSL::SSL
        Boost::system
        Boost::filesystem
)

add_executable(myapp src/main.cpp)
target_link_libraries(myapp PRIVATE mylib nlohmann_json::nlohmann_json)

enable_testing()
add_test(NAME mylib_test COMMAND mylib_test)
"#;

fn bench_parse_cmake_lists(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_cmake_lists");
    group.sample_size(50);

    let temp_dir = tempfile::tempdir().unwrap();
    std::fs::write(temp_dir.path().join("CMakeLists.txt"), CMAKE_CONTENT).unwrap();

    group.bench_function("complex", |b| {
        b.iter(|| {
            let deps = devbase::dependency_graph::extract_dependencies(temp_dir.path());
            black_box(deps);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_index_repo_full,
    bench_cosine_similarity,
    bench_extract_symbols,
    bench_parse_cmake_lists
);
criterion_main!(benches);
