//! Cross-repository dependency graph builder.
//!
//! Parses manifest files (Cargo.toml, package.json, go.mod) to discover
//! inter-repo dependencies within the local workspace and stores them in
//! `repo_relations` for AI-powered "which repo depends on X" queries.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, warn};

/// A dependency reference extracted from a manifest file.
#[derive(Debug, Clone)]
pub struct DependencyRef {
    pub name: String,
    pub kind: ManifestKind,
    /// If the dependency points to a local path (e.g. Cargo path dep).
    pub local_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ManifestKind {
    Cargo,
    Npm,
    Go,
    Python,
    CMake,
}

impl ManifestKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ManifestKind::Cargo => "cargo",
            ManifestKind::Npm => "npm",
            ManifestKind::Go => "go",
            ManifestKind::Python => "python",
            ManifestKind::CMake => "cmake",
        }
    }
}

/// Extract dependency references from a repository by inspecting its manifest files.
pub fn extract_dependencies(repo_path: &Path) -> Vec<DependencyRef> {
    let cargo_toml = repo_path.join("Cargo.toml");
    if cargo_toml.exists() {
        return parse_cargo_toml(&cargo_toml);
    }

    let package_json = repo_path.join("package.json");
    if package_json.exists() {
        return parse_package_json(&package_json, repo_path);
    }

    let go_mod = repo_path.join("go.mod");
    if go_mod.exists() {
        return parse_go_mod(&go_mod);
    }

    let pyproject = repo_path.join("pyproject.toml");
    if pyproject.exists() {
        return parse_pyproject_toml(&pyproject);
    }

    let requirements = repo_path.join("requirements.txt");
    if requirements.exists() {
        return parse_requirements_txt(&requirements);
    }

    let cmake_lists = repo_path.join("CMakeLists.txt");
    if cmake_lists.exists() {
        return parse_cmake_lists(&cmake_lists);
    }

    Vec::new()
}

fn parse_cargo_toml(path: &Path) -> Vec<DependencyRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let doc: toml::Table = match content.parse() {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to parse {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let mut deps = Vec::new();

    // Helper to extract deps from a table
    let mut extract_table = |table: &toml::Table| {
        for (name, value) in table {
            let local_path = match value {
                toml::Value::Table(t) => t.get("path").and_then(|p| p.as_str()).map(PathBuf::from),
                _ => None,
            };
            deps.push(DependencyRef {
                name: name.clone(),
                kind: ManifestKind::Cargo,
                local_path,
            });
        }
    };

    // [dependencies]
    if let Some(toml::Value::Table(table)) = doc.get("dependencies") {
        extract_table(table);
    }
    // [dev-dependencies]
    if let Some(toml::Value::Table(table)) = doc.get("dev-dependencies") {
        extract_table(table);
    }
    // [workspace.dependencies]
    if let Some(toml::Value::Table(ws)) = doc.get("workspace") {
        if let Some(toml::Value::Table(table)) = ws.get("dependencies") {
            extract_table(table);
        }
        // workspace members are local paths too
        if let Some(toml::Value::Array(members)) = ws.get("members") {
            for member in members {
                if let Some(path_str) = member.as_str() {
                    deps.push(DependencyRef {
                        name: path_str.to_string(),
                        kind: ManifestKind::Cargo,
                        local_path: Some(PathBuf::from(path_str)),
                    });
                }
            }
        }
    }

    deps
}

fn parse_package_json(path: &Path, _repo_path: &Path) -> Vec<DependencyRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let doc: serde_json::Value = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to parse {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let mut deps = Vec::new();

    for field in ["dependencies", "devDependencies", "peerDependencies"] {
        if let Some(obj) = doc.get(field).and_then(|v| v.as_object()) {
            for (name, value) in obj {
                let local_path =
                    value.as_str().and_then(|s| s.strip_prefix("file:").map(PathBuf::from));
                deps.push(DependencyRef {
                    name: name.clone(),
                    kind: ManifestKind::Npm,
                    local_path,
                });
            }
        }
    }

    // Monorepo workspace references
    if let Some(ws) = doc.get("workspaces").and_then(|v| v.as_array()) {
        for entry in ws {
            if let Some(path_str) = entry.as_str() {
                deps.push(DependencyRef {
                    name: path_str.to_string(),
                    kind: ManifestKind::Npm,
                    local_path: Some(PathBuf::from(path_str)),
                });
            }
        }
    }

    deps
}

fn parse_go_mod(path: &Path) -> Vec<DependencyRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let mut deps = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        // require (
        //     github.com/foo/bar v1.2.3
        // )
        if line.starts_with("require ") && !line.contains('(') {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                deps.push(DependencyRef {
                    name: parts[1].to_string(),
                    kind: ManifestKind::Go,
                    local_path: None,
                });
            }
        }
        // replace github.com/foo/bar => ../local/bar
        if line.starts_with("replace ") {
            let stripped = line.strip_prefix("replace ").unwrap_or(line);
            if let Some(pos) = stripped.find("=>") {
                let _old = stripped[..pos].trim();
                let new = stripped[pos + 2..].trim();
                // new may be a local path
                let local_path = if !new.starts_with("github.com/")
                    && !new.starts_with("golang.org/")
                    && !new.starts_with("google.golang.org/")
                {
                    Some(PathBuf::from(new))
                } else {
                    None
                };
                deps.push(DependencyRef {
                    name: _old.to_string(),
                    kind: ManifestKind::Go,
                    local_path,
                });
            }
        }
    }

    deps
}

fn parse_pyproject_toml(path: &Path) -> Vec<DependencyRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let doc: toml::Table = match content.parse() {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to parse {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let mut deps = Vec::new();

    // [project.dependencies] — PEP 621 (array of "name>=version" strings)
    if let Some(toml::Value::Array(arr)) = doc.get("project").and_then(|p| p.get("dependencies")) {
        for item in arr {
            if let Some(spec) = item.as_str() {
                let name = spec
                    .split(['=', '<', '>', '!', '~', ';'])
                    .next()
                    .unwrap_or(spec)
                    .trim()
                    .to_string();
                if !name.is_empty() {
                    deps.push(DependencyRef {
                        name,
                        kind: ManifestKind::Python,
                        local_path: None,
                    });
                }
            }
        }
    }

    // [tool.poetry.dependencies]
    if let Some(toml::Value::Table(poetry)) = doc
        .get("tool")
        .and_then(|t| t.get("poetry"))
        .and_then(|p| p.get("dependencies"))
    {
        for (name, value) in poetry {
            let local_path = match value {
                toml::Value::Table(t) => t.get("path").and_then(|p| p.as_str()).map(PathBuf::from),
                _ => None,
            };
            deps.push(DependencyRef {
                name: name.clone(),
                kind: ManifestKind::Python,
                local_path,
            });
        }
    }

    // [tool.uv.sources] with path = "..."
    if let Some(toml::Value::Table(sources)) =
        doc.get("tool").and_then(|t| t.get("uv")).and_then(|u| u.get("sources"))
    {
        for (name, value) in sources {
            let local_path = match value {
                toml::Value::Table(t) => t.get("path").and_then(|p| p.as_str()).map(PathBuf::from),
                _ => None,
            };
            deps.push(DependencyRef {
                name: name.clone(),
                kind: ManifestKind::Python,
                local_path,
            });
        }
    }

    deps
}

fn parse_requirements_txt(path: &Path) -> Vec<DependencyRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let mut deps = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with('-') {
            continue;
        }
        // Extract package name before version specifier
        let name = line
            .split(['=', '<', '>', '!', '[', ';'])
            .next()
            .unwrap_or(line)
            .trim()
            .to_string();
        if !name.is_empty() {
            deps.push(DependencyRef {
                name,
                kind: ManifestKind::Python,
                local_path: None,
            });
        }
    }

    deps
}

fn parse_cmake_lists(path: &Path) -> Vec<DependencyRef> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            warn!("Failed to read {:?}: {}", path, e);
            return Vec::new();
        }
    };

    let mut deps = Vec::new();
    let mut fetch_content_active = false;

    // CMake arguments can span multiple lines; merge logical lines so that
    // parentheses are balanced before we inspect a command.
    let mut buffer = String::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Remove inline comments
        let line_no_comment = trimmed.split('#').next().unwrap_or(trimmed).trim();
        if line_no_comment.is_empty() {
            continue;
        }

        if buffer.is_empty() {
            buffer.push_str(line_no_comment);
        } else {
            buffer.push(' ');
            buffer.push_str(line_no_comment);
        }

        // If parentheses are balanced, we have a complete command line.
        let open = buffer.chars().filter(|&c| c == '(').count();
        let close = buffer.chars().filter(|&c| c == ')').count();
        if open != close {
            continue; // wait for more lines
        }

        let line_no_comment = buffer.clone();
        buffer.clear();

        // find_package(NAME ...)
        if let Some(args) = line_no_comment.strip_prefix("find_package")
            && let Some(inner) = extract_parenthesized(args)
            && let Some(name) = inner.split_whitespace().next()
            && !name.is_empty()
        {
            deps.push(DependencyRef {
                name: name.to_string(),
                kind: ManifestKind::CMake,
                local_path: None,
            });
        }

        // add_subdirectory(path)
        if let Some(args) = line_no_comment.strip_prefix("add_subdirectory")
            && let Some(inner) = extract_parenthesized(args)
            && let Some(path_str) = inner.split_whitespace().next()
            && !path_str.is_empty()
        {
            let is_local = !path_str.starts_with("${")
                && !path_str.starts_with("http://")
                && !path_str.starts_with("https://");
            deps.push(DependencyRef {
                name: path_str.to_string(),
                kind: ManifestKind::CMake,
                local_path: if is_local {
                    Some(PathBuf::from(path_str))
                } else {
                    None
                },
            });
        }

        // include(FetchContent) — signal that following FetchContent_Declare should be captured
        if let Some(rest) = line_no_comment.strip_prefix("include")
            && let Some(inner) = extract_parenthesized(rest)
            && inner.trim() == "FetchContent"
        {
            fetch_content_active = true;
        }

        // FetchContent_Declare(name ...)
        if let Some(rest) = line_no_comment.strip_prefix("FetchContent_Declare")
            && let Some(inner) = extract_parenthesized(rest)
            && let Some(name) = inner.split_whitespace().next()
            && !name.is_empty()
        {
            deps.push(DependencyRef {
                name: name.to_string(),
                kind: ManifestKind::CMake,
                local_path: None,
            });
        }

        // target_link_libraries(target PRIVATE/PUBLIC/INTERFACE name)
        if let Some(args) = line_no_comment.strip_prefix("target_link_libraries")
            && let Some(inner) = extract_parenthesized(args)
        {
            let mut tokens = inner.split_whitespace().peekable();
            let _target = tokens.next(); // first token is target name
            while let Some(tok) = tokens.next() {
                let tok_upper = tok.to_uppercase();
                if (tok_upper == "PRIVATE" || tok_upper == "PUBLIC" || tok_upper == "INTERFACE")
                    && let Some(lib) = tokens.peek()
                    && !lib.starts_with("${")
                    && *lib != "${lib}"
                {
                    deps.push(DependencyRef {
                        name: (*lib).to_string(),
                        kind: ManifestKind::CMake,
                        local_path: None,
                    });
                    // advance past the library name we just consumed
                    let _ = tokens.next();
                }
            }
        }
    }

    // If FetchContent wasn't explicitly included but FetchContent_Declare exists,
    // we still captured it above. The flag is only for stricter behaviour if desired.
    let _ = fetch_content_active;

    deps
}

fn extract_parenthesized(s: &str) -> Option<&str> {
    let s = s.trim();
    let start = s.find('(')?;
    let mut depth = 0;
    for (i, c) in s[start..].chars().enumerate() {
        if c == '(' {
            depth += 1;
        } else if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Some(&s[start + 1..start + i]);
            }
        }
    }
    None
}

// ------------------------------------------------------------------
// Resolution against registered repos
// ------------------------------------------------------------------

/// Build dependency edges for a single repo and persist them.
pub fn build_dependency_graph(
    conn: &mut rusqlite::Connection,
    repo_id: &str,
    repo_path: &Path,
) -> anyhow::Result<usize> {
    let deps = extract_dependencies(repo_path);
    if deps.is_empty() {
        return Ok(0);
    }

    // Build a map of local_path -> repo_id for all registered repos
    let mut path_to_repo: HashMap<PathBuf, String> = HashMap::new();
    let mut name_to_repo: HashMap<String, String> = HashMap::new();

    {
        let mut stmt = conn.prepare(&format!(
            "SELECT id, local_path FROM entities WHERE entity_type = '{}'",
            crate::registry::ENTITY_TYPE_REPO
        ))?;
        let rows =
            stmt.query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))?;
        for row in rows {
            let (id, path_str) = row?;
            let pb = PathBuf::from(&path_str);
            path_to_repo.insert(pb.clone(), id.clone());
            // Also index by directory name
            if let Some(name) = pb.file_name().and_then(|n| n.to_str()) {
                name_to_repo.insert(name.to_string(), id.clone());
            }
        }
    }

    let mut resolved = 0;

    for dep in deps {
        // Strategy 1: local path dependency (highest confidence)
        if let Some(ref local_path) = dep.local_path {
            let abs_path = if local_path.is_absolute() {
                local_path.clone()
            } else {
                repo_path.join(local_path)
            };
            let canonical = std::fs::canonicalize(&abs_path).unwrap_or(abs_path.clone());

            if let Some(target_id) = path_to_repo.get(&canonical).or_else(|| {
                // Try without canonicalization
                path_to_repo.get(&abs_path)
            }) {
                crate::registry::WorkspaceRegistry::save_relation(
                    conn,
                    repo_id,
                    target_id,
                    "depends_on",
                    1.0,
                )?;
                resolved += 1;
                debug!("Resolved local path dep: {} -> {}", repo_id, target_id);
                continue;
            }

            // Try matching by directory name of the local path
            if let Some(dir_name) = canonical.file_name().and_then(|n| n.to_str())
                && let Some(target_id) = name_to_repo.get(dir_name)
            {
                crate::registry::WorkspaceRegistry::save_relation(
                    conn,
                    repo_id,
                    target_id,
                    "depends_on",
                    0.9,
                )?;
                resolved += 1;
                debug!("Resolved name-matched local path dep: {} -> {}", repo_id, target_id);
                continue;
            }
        }

        // Strategy 2: match dependency name against registered repo directory name
        if let Some(target_id) = name_to_repo.get(&dep.name) {
            crate::registry::WorkspaceRegistry::save_relation(
                conn,
                repo_id,
                target_id,
                "depends_on",
                0.7,
            )?;
            resolved += 1;
            debug!("Resolved name dep: {} -> {}", repo_id, target_id);
        }
    }

    Ok(resolved)
}

/// Query all outgoing dependencies for a repo.
pub fn list_dependencies(
    conn: &rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<Vec<(String, String, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT to_entity_id, relation_type, confidence FROM relations WHERE from_entity_id = ?1",
    )?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

/// Query all incoming dependencies (reverse deps) for a repo.
pub fn list_reverse_dependencies(
    conn: &rusqlite::Connection,
    repo_id: &str,
) -> anyhow::Result<Vec<(String, String, f64)>> {
    let mut stmt = conn.prepare(
        "SELECT from_entity_id, relation_type, confidence FROM relations WHERE to_entity_id = ?1",
    )?;
    let rows = stmt.query_map([repo_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, f64>(2)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
        path
    }

    #[test]
    fn test_parse_cargo_toml_deps() {
        let dir = tempfile::tempdir().unwrap();
        let cargo = r#"
[package]
name = "foo"

[dependencies]
serde = "1.0"
local-crate = { path = "../local-crate" }

[dev-dependencies]
tempfile = "3.0"
"#;
        write_file(dir.path(), "Cargo.toml", cargo);

        let deps = parse_cargo_toml(&dir.path().join("Cargo.toml"));
        assert_eq!(deps.len(), 3);
        let names: Vec<_> = deps.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"serde"));
        assert!(names.contains(&"local-crate"));
        assert!(names.contains(&"tempfile"));

        let local = deps.iter().find(|d| d.name == "local-crate").unwrap();
        assert_eq!(local.local_path, Some(PathBuf::from("../local-crate")));
    }

    #[test]
    fn test_parse_package_json_deps() {
        let dir = tempfile::tempdir().unwrap();
        let pkg = r#"
{
  "name": "my-app",
  "dependencies": {
    "lodash": "^4.0.0",
    "local-pkg": "file:../local-pkg"
  }
}
"#;
        write_file(dir.path(), "package.json", pkg);

        let deps = parse_package_json(&dir.path().join("package.json"), dir.path());
        assert_eq!(deps.len(), 2);
        let local = deps.iter().find(|d| d.name == "local-pkg").unwrap();
        assert_eq!(local.local_path, Some(PathBuf::from("../local-pkg")));
    }

    #[test]
    fn test_parse_go_mod_deps() {
        let dir = tempfile::tempdir().unwrap();
        let gomod = r#"
module github.com/example/app

go 1.21

require github.com/example/lib v1.0.0

replace github.com/example/lib => ../lib
"#;
        write_file(dir.path(), "go.mod", gomod);

        let deps = parse_go_mod(&dir.path().join("go.mod"));
        assert!(deps.iter().any(|d| d.name == "github.com/example/lib"));
        let local = deps
            .iter()
            .find(|d| d.name == "github.com/example/lib" && d.local_path.is_some())
            .unwrap();
        assert_eq!(local.local_path, Some(PathBuf::from("../lib")));
    }

    #[test]
    fn test_parse_pyproject_toml_deps() {
        let dir = tempfile::tempdir().unwrap();
        let pyproject = r#"
[project]
name = "my-app"
dependencies = [
    "requests>=2.0",
    "numpy",
]

[tool.poetry.dependencies]
fastapi = "^0.100"
local-pkg = { path = "../local-pkg" }

[tool.uv.sources]
local-pkg = { path = "../local-pkg" }
"#;
        write_file(dir.path(), "pyproject.toml", pyproject);

        let deps = parse_pyproject_toml(&dir.path().join("pyproject.toml"));
        assert!(deps.iter().any(|d| d.name == "requests"));
        assert!(deps.iter().any(|d| d.name == "numpy"));
        assert!(deps.iter().any(|d| d.name == "fastapi"));
        let local = deps.iter().find(|d| d.name == "local-pkg").unwrap();
        assert_eq!(local.local_path, Some(PathBuf::from("../local-pkg")));
    }

    #[test]
    fn test_parse_requirements_txt_deps() {
        let dir = tempfile::tempdir().unwrap();
        let req = r#"
# Comment line
requests>=2.0
numpy==1.24.0
fastapi[standard]>=0.100
-e ../editable-pkg
"#;
        write_file(dir.path(), "requirements.txt", req);

        let deps = parse_requirements_txt(&dir.path().join("requirements.txt"));
        assert!(deps.iter().any(|d| d.name == "requests"));
        assert!(deps.iter().any(|d| d.name == "numpy"));
        assert!(deps.iter().any(|d| d.name == "fastapi"));
        // -e lines are skipped
        assert!(!deps.iter().any(|d| d.name == "-e"));
    }

    #[test]
    fn test_parse_cmake_find_package() {
        let dir = tempfile::tempdir().unwrap();
        let cmake = r#"
cmake_minimum_required(VERSION 3.20)
project(demo)

# External deps
find_package(Boost REQUIRED)
find_package(Threads)
find_package(OpenSSL 1.1 REQUIRED)
"#;
        write_file(dir.path(), "CMakeLists.txt", cmake);
        let deps = parse_cmake_lists(&dir.path().join("CMakeLists.txt"));
        assert!(deps.iter().any(|d| d.name == "Boost"));
        assert!(deps.iter().any(|d| d.name == "Threads"));
        assert!(deps.iter().any(|d| d.name == "OpenSSL"));
    }

    #[test]
    fn test_parse_cmake_add_subdirectory_local() {
        let dir = tempfile::tempdir().unwrap();
        let cmake = r#"
add_subdirectory(third_party/fmt)
add_subdirectory(${EXTERNAL_DIR}/foo) # variable, not local path dep
add_subdirectory(https://example.com/repo) # remote, no local_path
"#;
        write_file(dir.path(), "CMakeLists.txt", cmake);
        let deps = parse_cmake_lists(&dir.path().join("CMakeLists.txt"));
        let local = deps.iter().find(|d| d.name == "third_party/fmt").unwrap();
        assert_eq!(local.local_path, Some(PathBuf::from("third_party/fmt")));
        let var = deps.iter().find(|d| d.name == "${EXTERNAL_DIR}/foo").unwrap();
        assert!(var.local_path.is_none());
        let remote = deps.iter().find(|d| d.name == "https://example.com/repo").unwrap();
        assert!(remote.local_path.is_none());
    }

    #[test]
    fn test_parse_cmake_fetchcontent_declare() {
        let dir = tempfile::tempdir().unwrap();
        let cmake = r#"
include(FetchContent)
FetchContent_Declare(
  googletest
  GIT_REPOSITORY https://github.com/google/googletest.git
  GIT_TAG v1.14.0
)
FetchContent_Declare(nlohmann_json URL https://github.com/nlohmann/json/releases/download/v3.11.2/json.tar.xz)
"#;
        write_file(dir.path(), "CMakeLists.txt", cmake);
        let deps = parse_cmake_lists(&dir.path().join("CMakeLists.txt"));
        assert!(deps.iter().any(|d| d.name == "googletest"));
        assert!(deps.iter().any(|d| d.name == "nlohmann_json"));
    }

    #[test]
    fn test_parse_cmake_target_link_libraries() {
        let dir = tempfile::tempdir().unwrap();
        let cmake = r#"
add_executable(app main.cpp)
target_link_libraries(app PRIVATE Boost::filesystem PUBLIC Threads::Threads INTERFACE fmt::fmt)
"#;
        write_file(dir.path(), "CMakeLists.txt", cmake);
        let deps = parse_cmake_lists(&dir.path().join("CMakeLists.txt"));
        assert!(deps.iter().any(|d| d.name == "Boost::filesystem"));
        assert!(deps.iter().any(|d| d.name == "Threads::Threads"));
        assert!(deps.iter().any(|d| d.name == "fmt::fmt"));
    }
}
