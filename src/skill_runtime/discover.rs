//! Auto-discovery: analyze a GitHub project and generate a SKILL.md draft + entry_script wrapper.
//!
//! Phase 1 scope:
//! - Detect project type from manifest files (Cargo.toml, package.json, pyproject.toml, go.mod, etc.)
//! - Extract CLI surface (bin targets, scripts, Makefile targets)
//! - Extract API surface (openapi spec, graphql schema, RPC definitions)
//! - Generate SKILL.md with inferred inputs/outputs schema
//! - Generate a Python entry_script wrapper that maps Skill I/O to project CLI/API
//! - Register as a Skill in the registry
//!
//! Design constraints (general-market, not personal):
//! - Do NOT assume Rust/Cargo specifically
//! - Do NOT hardcode workspace paths
//! - Do NOT optimize for the 50-project reference set

use super::{SkillInput, SkillMeta, SkillOutput, SkillType};
use anyhow::Context;
use std::path::{Path, PathBuf};

/// Project type detected from manifest files.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Docker,
    #[default]
    Generic,
}

/// Detected CLI/API surface of a project.
#[derive(Debug, Clone, Default)]
pub struct ProjectSurface {
    pub project_type: ProjectType,
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub repo_url: Option<String>,
    /// CLI commands / subcommands extracted from the project.
    pub cli_commands: Vec<CliCommand>,
    /// API endpoints / methods (if detectable).
    pub api_methods: Vec<ApiMethod>,
    /// Available environment variables or config keys.
    pub env_vars: Vec<EnvVarHint>,
    /// Language/tooling tags (e.g. "rust", "cli", "api", "docker").
    pub tags: Vec<String>,
    /// Path to the original manifest file used for detection.
    pub manifest_path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct CliCommand {
    pub name: String,
    pub description: String,
    pub args: Vec<SkillInput>,
}

#[derive(Debug, Clone)]
pub struct ApiMethod {
    pub name: String,
    pub method_type: String, // "http", "grpc", "graphql", etc.
    pub description: String,
    pub inputs: Vec<SkillInput>,
    pub outputs: Vec<SkillOutput>,
}

#[derive(Debug, Clone)]
pub struct EnvVarHint {
    pub name: String,
    pub description: String,
    pub required: bool,
}

/// Analyze a project directory and discover its CLI/API surface.
pub fn analyze_project(path: &Path) -> anyhow::Result<ProjectSurface> {
    let abs_path = std::fs::canonicalize(path)?;

    // Detection order: most specific to least specific
    let mut surface = ProjectSurface {
        manifest_path: Some(abs_path.clone()),
        ..Default::default()
    };

    if let Some(cargo) = try_read_cargo_toml(&abs_path)? {
        surface.project_type = ProjectType::Rust;
        surface.name = cargo.name;
        surface.version = cargo.version;
        surface.description = cargo.description;
        surface.authors = cargo.authors;
        surface.repo_url = cargo.repository;
        surface.cli_commands = cargo
            .bin_targets
            .into_iter()
            .map(|(name, desc)| CliCommand {
                name,
                description: desc,
                args: vec![],
            })
            .collect();
        surface.tags = vec!["rust".into()];
        if !surface.cli_commands.is_empty() {
            surface.tags.push("cli".into());
        }
        return Ok(surface);
    }

    if let Some(pkg) = try_read_package_json(&abs_path)? {
        surface.project_type = ProjectType::Node;
        surface.name = pkg.name;
        surface.version = pkg.version;
        surface.description = pkg.description;
        surface.repo_url = pkg.repository;
        surface.cli_commands = pkg
            .scripts
            .into_iter()
            .map(|(name, cmd)| CliCommand {
                name,
                description: format!("npm/yarn script: {}", cmd),
                args: vec![SkillInput {
                    name: "args".into(),
                    input_type: "string".into(),
                    description: "Additional arguments to pass".into(),
                    required: false,
                    default: None,
                }],
            })
            .collect();
        surface.tags = vec!["node".into(), "javascript".into()];
        if pkg.has_bin {
            surface.tags.push("cli".into());
        }
        return Ok(surface);
    }

    if let Some(py) = try_read_python_project(&abs_path)? {
        surface.project_type = ProjectType::Python;
        surface.name = py.name;
        surface.version = py.version;
        surface.description = py.description;
        surface.repo_url = py.repository;
        surface.cli_commands = py
            .scripts
            .into_iter()
            .map(|(name, entry)| CliCommand {
                name,
                description: format!("Python console script: {}", entry),
                args: vec![SkillInput {
                    name: "args".into(),
                    input_type: "string".into(),
                    description: "Command-line arguments".into(),
                    required: false,
                    default: None,
                }],
            })
            .collect();
        surface.tags = vec!["python".into()];
        return Ok(surface);
    }

    if let Some(go) = try_read_go_mod(&abs_path)? {
        surface.project_type = ProjectType::Go;
        surface.name = go.module_name.rsplit('/').next().unwrap_or("go-project").to_string();
        surface.version = "0.0.0".to_string();
        surface.description = format!("Go module: {}", go.module_name);
        surface.tags = vec!["go".into()];
        return Ok(surface);
    }

    if abs_path.join("Dockerfile").exists() || abs_path.join("docker-compose.yml").exists() {
        surface.project_type = ProjectType::Docker;
        surface.name = abs_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("docker-project")
            .to_string();
        surface.version = "0.0.0".to_string();
        surface.description = "Docker-based project".to_string();
        surface.tags = vec!["docker".into()];
        return Ok(surface);
    }

    // Fallback: generic project
    surface.project_type = ProjectType::Generic;
    surface.name = abs_path.file_name().and_then(|n| n.to_str()).unwrap_or("project").to_string();
    surface.version = "0.0.0".to_string();
    surface.description = try_read_readme_summary(&abs_path).unwrap_or_default();
    surface.tags = vec!["generic".into()];

    Ok(surface)
}

/// Generate a SKILL.md draft from discovered project surface.
pub fn generate_skill_md(surface: &ProjectSurface, skill_id: &str) -> String {
    let mut lines = Vec::new();
    lines.push("---".to_string());
    lines.push(format!("id: {}", skill_id));
    lines.push(format!("name: {}", surface.name));
    lines.push(format!("version: {}", surface.version));
    lines.push(format!("description: {}", surface.description));
    if let Some(ref author) = surface.authors.first() {
        lines.push(format!("author: {}", author));
    }
    if !surface.tags.is_empty() {
        lines.push(format!(
            "tags: [{}]",
            surface.tags.iter().map(|t| format!("'{}'", t)).collect::<Vec<_>>().join(", ")
        ));
    }
    lines.push("skill_type: custom".to_string());
    lines.push("entry_script: scripts/run.py".to_string());

    // Inputs: project path + optional command selection
    lines.push("inputs:".to_string());
    lines.push("  - name: command".to_string());
    lines.push("    type: string".to_string());
    lines.push("    description: CLI command or script to execute".to_string());
    lines.push("    required: true".to_string());
    if let Some(first) = surface.cli_commands.first() {
        lines.push(format!("    default: '{}'", first.name));
    }
    lines.push("  - name: args".to_string());
    lines.push("    type: string".to_string());
    lines.push("    description: Arguments to pass to the command".to_string());
    lines.push("    required: false".to_string());
    lines.push("  - name: working_dir".to_string());
    lines.push("    type: string".to_string());
    lines.push("    description: Working directory for execution".to_string());
    lines.push("    required: false".to_string());

    // Outputs
    lines.push("outputs:".to_string());
    lines.push("  - name: stdout".to_string());
    lines.push("    type: string".to_string());
    lines.push("    description: Standard output of the command".to_string());
    lines.push("  - name: stderr".to_string());
    lines.push("    type: string".to_string());
    lines.push("    description: Standard error of the command".to_string());
    lines.push("  - name: exit_code".to_string());
    lines.push("    type: integer".to_string());
    lines.push("    description: Exit code of the command".to_string());

    lines.push("---".to_string());
    lines.push("".to_string());

    // Body: auto-generated documentation
    lines.push(format!("# Skill: {}", surface.name));
    lines.push("".to_string());
    lines.push("## Overview".to_string());
    lines.push(format!(
        "This Skill was auto-discovered from a {} project.",
        project_type_name(&surface.project_type)
    ));
    lines.push("".to_string());
    if let Some(ref url) = surface.repo_url {
        lines.push(format!("- **Repository**: {}", url));
    }
    lines.push(format!("- **Version**: {}", surface.version));
    lines.push("".to_string());

    if !surface.cli_commands.is_empty() {
        lines.push("## Available Commands".to_string());
        for cmd in &surface.cli_commands {
            lines.push(format!("- `{}`: {}", cmd.name, cmd.description));
        }
        lines.push("".to_string());
    }

    if !surface.api_methods.is_empty() {
        lines.push("## API Methods".to_string());
        for api in &surface.api_methods {
            lines.push(format!("- `{}` ({}): {}", api.name, api.method_type, api.description));
        }
        lines.push("".to_string());
    }

    lines.push("## Auto-Generated".to_string());
    lines.push("This SKILL.md was generated by `devbase skill discover`.".to_string());
    lines.push("Human review and refinement are recommended before publishing.".to_string());

    lines.join("\n")
}

/// Generate a Python entry_script wrapper.
pub fn generate_entry_script(surface: &ProjectSurface, project_root: &Path) -> String {
    let root_str = project_root.to_string_lossy().replace('\\', "/");
    let mut script = String::new();
    script.push_str("#!/usr/bin/env python3\n");
    script.push_str("\"\"\"Auto-generated entry script for devbase Skill execution.\"\"\"\n");
    script.push_str("import json\n");
    script.push_str("import sys\n");
    script.push_str("import os\n");
    script.push_str("import subprocess\n");
    script.push('\n');
    script.push_str(&format!("PROJECT_ROOT = '{}'\n", root_str));
    script.push('\n');
    script.push_str("def main():\n");
    script.push_str("    # Read input from stdin or args\n");
    script.push_str("    if len(sys.argv) > 1:\n");
    script.push_str("        try:\n");
    script.push_str("            inp = json.loads(sys.argv[1])\n");
    script.push_str("        except json.JSONDecodeError:\n");
    script.push_str("            inp = {'command': sys.argv[1], 'args': ' '.join(sys.argv[2:])}\n");
    script.push_str("    else:\n");
    script.push_str("        raw = sys.stdin.read()\n");
    script.push_str("        inp = json.loads(raw) if raw.strip() else {}\n");
    script.push('\n');
    script.push_str("    command = inp.get('command', '')\n");
    script.push_str("    args = inp.get('args', '')\n");
    script.push_str("    project_root = inp.get('working_dir', PROJECT_ROOT)\n");
    script.push('\n');

    // Project-type-specific execution logic
    match surface.project_type {
        ProjectType::Rust => {
            script.push_str("    cmd = ['cargo', 'run', '--bin', command]\n");
            script.push_str("    if args:\n");
            script.push_str("        cmd.extend(args.split())\n");
            script.push_str("    result = subprocess.run(cmd, cwd=project_root, capture_output=True, text=True)\n");
        }
        ProjectType::Node => {
            script.push_str("    # Try npm script first, then npx/bin\n");
            script.push_str("    if os.path.exists(os.path.join(project_root, 'package.json')):\n");
            script.push_str("        cmd = ['npm', 'run', command]\n");
            script.push_str("    else:\n");
            script.push_str("        cmd = ['npx', command]\n");
            script.push_str("    if args:\n");
            script.push_str("        cmd.extend(args.split())\n");
            script.push_str("    result = subprocess.run(cmd, cwd=project_root, capture_output=True, text=True)\n");
        }
        ProjectType::Python => {
            script.push_str("    # Try python -m first, then direct script\n");
            script.push_str("    module_path = command.replace('-', '_')\n");
            script.push_str("    cmd = [sys.executable, '-m', module_path]\n");
            script.push_str("    if args:\n");
            script.push_str("        cmd.extend(args.split())\n");
            script.push_str("    result = subprocess.run(cmd, cwd=project_root, capture_output=True, text=True)\n");
        }
        ProjectType::Go => {
            script.push_str("    cmd = ['go', 'run', '.', command]\n");
            script.push_str("    if args:\n");
            script.push_str("        cmd.extend(args.split())\n");
            script.push_str("    result = subprocess.run(cmd, cwd=project_root, capture_output=True, text=True)\n");
        }
        ProjectType::Docker => {
            script.push_str("    cmd = ['docker', 'compose', 'run', '--rm', command]\n");
            script.push_str("    if args:\n");
            script.push_str("        cmd.extend(args.split())\n");
            script.push_str("    result = subprocess.run(cmd, cwd=project_root, capture_output=True, text=True)\n");
        }
        ProjectType::Generic => {
            script.push_str("    cmd = command.split() if command else []\n");
            script.push_str("    if args:\n");
            script.push_str("        cmd.extend(args.split())\n");
            script.push_str("    if not cmd:\n");
            script.push_str("        print(json.dumps({'error': 'No command specified'}))\n");
            script.push_str("        sys.exit(1)\n");
            script.push_str("    result = subprocess.run(cmd, cwd=project_root, capture_output=True, text=True)\n");
        }
    }

    script.push('\n');
    script.push_str("    output = {\n");
    script.push_str("        'stdout': result.stdout,\n");
    script.push_str("        'stderr': result.stderr,\n");
    script.push_str("        'exit_code': result.returncode,\n");
    script.push_str("    }\n");
    script.push_str("    print(json.dumps(output, ensure_ascii=False))\n");
    script.push_str("    sys.exit(0 if result.returncode == 0 else 1)\n");
    script.push('\n');
    script.push_str("if __name__ == '__main__':\n");
    script.push_str("    main()\n");

    script
}

/// Discover a project, generate SKILL.md + entry_script, and install as a Skill.
pub fn discover_and_install(
    conn: &rusqlite::Connection,
    project_path: &Path,
    skill_id: Option<&str>,
    dry_run: bool,
) -> anyhow::Result<SkillMeta> {
    let surface = analyze_project(project_path)?;
    let category = infer_category(&surface);

    let id = skill_id.map(|s| s.to_string()).unwrap_or_else(|| {
        surface
            .name
            .chars()
            .map(|c| if c == ' ' || c == '_' { '-' } else { c })
            .collect()
    });

    let skills_dir = crate::registry::WorkspaceRegistry::workspace_dir()?.join("skills");
    let skill_dir = skills_dir.join(&id);

    if !dry_run {
        std::fs::create_dir_all(&skill_dir)?;
        let scripts_dir = skill_dir.join("scripts");
        std::fs::create_dir_all(&scripts_dir)?;
    }

    // Determine project root for the entry script.
    // For local paths: point back to the original source directory.
    // For git-cloned paths (where project_path == skill_dir): point to skill_dir itself.
    let project_root = if project_path == skill_dir.as_path() {
        skill_dir.clone()
    } else {
        std::fs::canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf())
    };

    // Generate SKILL.md
    let skill_md = generate_skill_md(&surface, &id);
    let skill_md_path = skill_dir.join("SKILL.md");
    if dry_run {
        println!("=== SKILL.md (dry-run) ===");
        println!("{}", skill_md);
    } else {
        std::fs::write(&skill_md_path, &skill_md)?;
    }

    // Generate entry script
    let entry_script = generate_entry_script(&surface, &project_root);
    let entry_path = skill_dir.join("scripts").join("run.py");
    if dry_run {
        println!("\n=== entry_script (dry-run) ===");
        println!("{}", entry_script);
    } else {
        std::fs::write(&entry_path, &entry_script)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&entry_path)?.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&entry_path, perms)?;
        }
    }

    // Parse the generated SKILL.md into SkillMeta
    let skill = if dry_run {
        // In dry-run mode, construct SkillMeta manually without file I/O
        let now = chrono::Utc::now();
        SkillMeta {
            id: id.clone(),
            name: surface.name.clone(),
            version: surface.version.clone(),
            description: surface.description.clone(),
            author: surface.authors.first().cloned(),
            tags: surface.tags.clone(),
            entry_script: Some("scripts/run.py".to_string()),
            skill_type: SkillType::Custom,
            category: category.clone(),
            local_path: skill_dir.clone(),
            inputs: vec![
                SkillInput {
                    name: "command".into(),
                    input_type: "string".into(),
                    description: "CLI command or script to execute".into(),
                    required: true,
                    default: surface.cli_commands.first().map(|c| c.name.clone()),
                },
                SkillInput {
                    name: "args".into(),
                    input_type: "string".into(),
                    description: "Arguments to pass to the command".into(),
                    required: false,
                    default: None,
                },
                SkillInput {
                    name: "working_dir".into(),
                    input_type: "string".into(),
                    description: "Working directory for execution".into(),
                    required: false,
                    default: None,
                },
            ],
            outputs: vec![
                SkillOutput {
                    name: "stdout".into(),
                    output_type: "string".into(),
                    description: "Standard output".into(),
                },
                SkillOutput {
                    name: "stderr".into(),
                    output_type: "string".into(),
                    description: "Standard error".into(),
                },
                SkillOutput {
                    name: "exit_code".into(),
                    output_type: "integer".into(),
                    description: "Exit code".into(),
                },
            ],
            dependencies: vec![],
            embedding: None,
            installed_at: now,
            updated_at: now,
            last_used_at: None,
            body: skill_md,
        }
    } else {
        let mut skill = crate::skill_runtime::parser::parse_skill_md(&skill_md_path)?;
        skill.category = category.clone();
        skill
    };

    if !dry_run {
        crate::skill_runtime::registry::install_skill(conn, &skill)?;
    }

    Ok(skill)
}

// ---------------------------------------------------------------------------
// Manifest parsers (project-type detection)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct CargoToml {
    name: String,
    version: String,
    description: String,
    authors: Vec<String>,
    repository: Option<String>,
    bin_targets: Vec<(String, String)>,
}

fn try_read_cargo_toml(path: &Path) -> anyhow::Result<Option<CargoToml>> {
    let cargo_path = path.join("Cargo.toml");
    if !cargo_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&cargo_path)?;
    let doc: toml::Value = content.parse().context("parse Cargo.toml")?;

    let package = doc.get("package").and_then(|p| p.as_table());
    let Some(pkg) = package else {
        return Ok(None);
    };

    let name = pkg.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let version = pkg.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0").to_string();
    let description = pkg.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let authors: Vec<String> = pkg
        .get("authors")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
        .unwrap_or_default();
    let repository = pkg.get("repository").and_then(|v| v.as_str()).map(|s| s.to_string());

    let mut bin_targets = Vec::new();
    if let Some(bins) = doc.get("bin").and_then(|b| b.as_array()) {
        for bin in bins {
            if let Some(table) = bin.as_table() {
                let bin_name = table.get("name").and_then(|v| v.as_str()).unwrap_or(&name);
                bin_targets.push((bin_name.to_string(), format!("Binary target: {}", bin_name)));
            }
        }
    }
    // If no explicit [[bin]], default to package name
    if bin_targets.is_empty() {
        bin_targets.push((name.clone(), format!("Default binary: {}", name)));
    }

    Ok(Some(CargoToml {
        name,
        version,
        description,
        authors,
        repository,
        bin_targets,
    }))
}

#[derive(Debug, Clone, Default)]
struct PackageJson {
    name: String,
    version: String,
    description: String,
    repository: Option<String>,
    scripts: Vec<(String, String)>,
    has_bin: bool,
}

fn try_read_package_json(path: &Path) -> anyhow::Result<Option<PackageJson>> {
    let pkg_path = path.join("package.json");
    if !pkg_path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&pkg_path)?;
    let doc: serde_json::Value = serde_json::from_str(&content).context("parse package.json")?;

    let name = doc.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
    let version = doc.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0").to_string();
    let description = doc.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let repository = doc
        .get("repository")
        .and_then(|v| v.as_str().or_else(|| v.get("url").and_then(|u| u.as_str())))
        .map(|s| s.to_string());

    let mut scripts = Vec::new();
    if let Some(scr) = doc.get("scripts").and_then(|v| v.as_object()) {
        for (k, v) in scr {
            if let Some(cmd) = v.as_str() {
                scripts.push((k.clone(), cmd.to_string()));
            }
        }
    }

    let has_bin = doc.get("bin").is_some();

    Ok(Some(PackageJson {
        name,
        version,
        description,
        repository,
        scripts,
        has_bin,
    }))
}

#[derive(Debug, Clone, Default)]
struct PythonProject {
    name: String,
    version: String,
    description: String,
    repository: Option<String>,
    scripts: Vec<(String, String)>,
}

fn try_read_python_project(path: &Path) -> anyhow::Result<Option<PythonProject>> {
    // Try pyproject.toml first (PEP 621)
    let pyproject = path.join("pyproject.toml");
    if pyproject.exists() {
        let content = std::fs::read_to_string(&pyproject)?;
        let doc: toml::Value = content.parse().context("parse pyproject.toml")?;

        let project = doc.get("project").and_then(|p| p.as_table());
        if let Some(proj) = project {
            let name = proj.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
            let version =
                proj.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0").to_string();
            let description =
                proj.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let repository = proj
                .get("urls")
                .and_then(|u| u.as_table())
                .and_then(|t| t.get("Repository").or_else(|| t.get("repository")))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let mut scripts = Vec::new();
            if let Some(scripts_table) = proj.get("scripts").and_then(|v| v.as_table()) {
                for (k, v) in scripts_table {
                    if let Some(entry) = v.as_str() {
                        scripts.push((k.clone(), entry.to_string()));
                    }
                }
            }

            return Ok(Some(PythonProject {
                name,
                version,
                description,
                repository,
                scripts,
            }));
        }
    }

    // Fallback: setup.py detection (minimal)
    let setup_py = path.join("setup.py");
    if setup_py.exists() {
        let content = std::fs::read_to_string(&setup_py)?;
        // Very naive extraction
        let name =
            extract_setup_py_kwarg(&content, "name").unwrap_or_else(|| "unknown".to_string());
        let version =
            extract_setup_py_kwarg(&content, "version").unwrap_or_else(|| "0.0.0".to_string());
        return Ok(Some(PythonProject {
            name,
            version,
            description: "Python project (setup.py)".to_string(),
            repository: None,
            scripts: vec![],
        }));
    }

    Ok(None)
}

fn extract_setup_py_kwarg(content: &str, key: &str) -> Option<String> {
    let pattern = format!("{}='", key);
    if let Some(start) = content.find(&pattern) {
        let after = &content[start + pattern.len()..];
        if let Some(end) = after.find('\'') {
            return Some(after[..end].to_string());
        }
    }
    let pattern = format!("{}=\"", key);
    if let Some(start) = content.find(&pattern) {
        let after = &content[start + pattern.len()..];
        if let Some(end) = after.find('"') {
            return Some(after[..end].to_string());
        }
    }
    None
}

#[derive(Debug, Clone, Default)]
struct GoMod {
    module_name: String,
}

fn try_read_go_mod(path: &Path) -> anyhow::Result<Option<GoMod>> {
    let go_mod = path.join("go.mod");
    if !go_mod.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&go_mod)?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(name) = trimmed.strip_prefix("module ") {
            return Ok(Some(GoMod {
                module_name: name.trim().to_string(),
            }));
        }
    }
    Ok(None)
}

fn try_read_readme_summary(path: &Path) -> Option<String> {
    for name in &["README.md", "readme.md", "Readme.md"] {
        let readme = path.join(name);
        if let Ok(content) = std::fs::read_to_string(&readme) {
            // First non-empty line after optional title
            let summary =
                content.lines().map(|l| l.trim()).find(|l| !l.is_empty() && !l.starts_with('#'));
            return summary.map(|s| s.to_string());
        }
    }
    None
}

fn project_type_name(pt: &ProjectType) -> &'static str {
    match pt {
        ProjectType::Rust => "Rust",
        ProjectType::Node => "Node.js",
        ProjectType::Python => "Python",
        ProjectType::Go => "Go",
        ProjectType::Docker => "Docker",
        ProjectType::Generic => "generic",
    }
}

/// Infer taxonomy category from project surface.
///
/// Primary categories: ai, dev, data, infra, communication
/// Secondary (sub-categories) use "/" separator, e.g. "dev/cli", "ai/llm"
fn infer_category(surface: &ProjectSurface) -> Option<String> {
    let name_lower = surface.name.to_lowercase();
    let desc_lower = surface.description.to_lowercase();
    let combined = format!("{} {}", name_lower, desc_lower);
    let _tags: Vec<String> = surface.tags.iter().map(|t| t.to_lowercase()).collect();

    // AI / ML
    let ai_keywords = [
        "ai",
        "llm",
        "model",
        "gpt",
        "neural",
        "ml",
        "machine learning",
        "embedding",
        "vector",
        "rag",
        "agent",
        "claude",
        "openai",
    ];
    if ai_keywords.iter().any(|k| combined.contains(k)) {
        if combined.contains("agent") || combined.contains("orchestr") {
            return Some("ai/agent".to_string());
        }
        if combined.contains("llm") || combined.contains("gpt") || combined.contains("model") {
            return Some("ai/llm".to_string());
        }
        return Some("ai".to_string());
    }

    // Data
    let data_keywords = [
        "data",
        "database",
        "sql",
        "etl",
        "pipeline",
        "csv",
        "json",
        "parquet",
        "analytics",
        "warehouse",
    ];
    if data_keywords.iter().any(|k| combined.contains(k)) {
        return Some("data".to_string());
    }

    // Infrastructure
    let infra_keywords = [
        "infra",
        "server",
        "deploy",
        "docker",
        "kubernetes",
        "k8s",
        "cloud",
        "aws",
        "monitor",
        "logging",
        "observability",
    ];
    if infra_keywords.iter().any(|k| combined.contains(k))
        || surface.project_type == ProjectType::Docker
    {
        return Some("infra".to_string());
    }

    // Communication / Collaboration
    let comm_keywords = [
        "chat", "message", "slack", "discord", "email", "notify", "alert", "gateway", "bridge",
        "protocol",
    ];
    if comm_keywords.iter().any(|k| combined.contains(k)) {
        return Some("communication".to_string());
    }

    // Development (default fallback for most projects)
    let dev_keywords = [
        "dev",
        "code",
        "lint",
        "format",
        "test",
        "build",
        "compiler",
        "ide",
        "editor",
        "git",
        "version control",
    ];
    if dev_keywords.iter().any(|k| combined.contains(k)) {
        if surface.project_type == ProjectType::Rust || surface.project_type == ProjectType::Go {
            return Some("dev/toolchain".to_string());
        }
        if !surface.cli_commands.is_empty() {
            return Some("dev/cli".to_string());
        }
        return Some("dev".to_string());
    }

    // Default based on project type
    match surface.project_type {
        ProjectType::Rust | ProjectType::Go | ProjectType::Node | ProjectType::Python => {
            if !surface.cli_commands.is_empty() {
                Some("dev/cli".to_string())
            } else {
                Some("dev".to_string())
            }
        }
        ProjectType::Docker => Some("infra".to_string()),
        ProjectType::Generic => None,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_skill_md_structure() {
        let surface = ProjectSurface {
            project_type: ProjectType::Rust,
            name: "my-tool".into(),
            version: "1.0.0".into(),
            description: "A useful tool".into(),
            authors: vec!["Alice".into()],
            repo_url: Some("https://github.com/alice/my-tool".into()),
            cli_commands: vec![CliCommand {
                name: "my-tool".into(),
                description: "Main binary".into(),
                args: vec![],
            }],
            api_methods: vec![],
            env_vars: vec![],
            tags: vec!["rust".into(), "cli".into()],
            manifest_path: None,
        };
        let md = generate_skill_md(&surface, "my-tool");
        assert!(md.contains("id: my-tool"));
        assert!(md.contains("name: my-tool"));
        assert!(md.contains("A useful tool"));
        assert!(md.contains("entry_script: scripts/run.py"));
        assert!(md.contains("inputs:"));
        assert!(md.contains("outputs:"));
        assert!(md.contains("Auto-Generated"));
    }

    #[test]
    fn test_generate_entry_script_rust() {
        let surface = ProjectSurface {
            project_type: ProjectType::Rust,
            ..Default::default()
        };
        let tmp = std::env::temp_dir();
        let script = generate_entry_script(&surface, &tmp);
        assert!(script.contains("'cargo'"));
        assert!(script.contains("'run'"));
        assert!(script.contains("subprocess.run"));
        assert!(script.contains("PROJECT_ROOT"));
    }

    #[test]
    fn test_generate_entry_script_node() {
        let surface = ProjectSurface {
            project_type: ProjectType::Node,
            ..Default::default()
        };
        let tmp = std::env::temp_dir();
        let script = generate_entry_script(&surface, &tmp);
        assert!(script.contains("npm run") || script.contains("npx"));
    }

    #[test]
    fn test_generate_entry_script_python() {
        let surface = ProjectSurface {
            project_type: ProjectType::Python,
            ..Default::default()
        };
        let tmp = std::env::temp_dir();
        let script = generate_entry_script(&surface, &tmp);
        assert!(script.contains("python -m") || script.contains("sys.executable"));
    }
}
