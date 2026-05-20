// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use crate::mcp::McpTool;
use crate::storage::AppContext;
use anyhow::Context;
use std::path::Path;

#[derive(Clone)]
pub struct DevkitDocumentConvertTool;

impl McpTool for DevkitDocumentConvertTool {
    fn name(&self) -> &'static str {
        "devkit_document_convert"
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "description": r#"Convert PDF/PPTX documents to Markdown text.

Use this when the user wants to:
- Extract text content from course materials, papers, or slides
- Convert binary documents into editable Markdown for the Vault
- Bulk-process downloaded files before organizing them

Supported formats:
- PDF (via pdftotext)
- PPTX (via python-pptx)

Parameters:
- source_path: Absolute path to the source document
- output_path: Optional absolute path for the output Markdown file. Defaults to source_path with .md extension.

Returns: JSON with output_path, extracted character count, and a quality hint (good / poor)."#,
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source_path": { "type": "string", "description": "Absolute path to the source document" },
                    "output_path": { "type": "string", "description": "Optional absolute path for output Markdown" }
                },
                "required": ["source_path"]
            }
        })
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        _ctx: &mut AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let source_path = args
            .get("source_path")
            .and_then(|v| v.as_str())
            .context("Missing required argument: source_path")?;

        let source = Path::new(source_path);
        anyhow::ensure!(source.exists(), "Source file not found: {}", source_path);

        let output_path = args
            .get("output_path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| source.with_extension("md").to_string_lossy().to_string());

        let ext = source.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

        let (text, quality) = match ext.as_str() {
            "pdf" => extract_pdf(source_path).await?,
            "pptx" | "ppt" => extract_pptx(source_path).await?,
            other => anyhow::bail!("Unsupported file format: '{}' (supported: pdf, pptx)", other),
        };

        let cleaned = cleanup_extracted_text(&text);
        let frontmatter =
            format!("---\nsource: \"{}\"\nextract_quality: \"{}\"\n---\n\n", source_path, quality);
        let md_content = format!("{}{}", frontmatter, cleaned);

        std::fs::write(&output_path, md_content)
            .with_context(|| format!("Failed to write output: {}", output_path))?;

        Ok(serde_json::json!({
            "success": true,
            "output_path": output_path,
            "extracted_chars": text.len(),
            "quality": quality,
        }))
    }
}

async fn extract_pdf(path: &str) -> anyhow::Result<(String, &'static str)> {
    let output = tokio::process::Command::new("pdftotext")
        .args(["-", path, "-"]) // read from file, write to stdout
        .output()
        .await
        .context("Failed to spawn pdftotext — is poppler installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("pdftotext failed: {}", stderr);
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    // Heuristic: if output is very short relative to file size, quality is poor
    let quality = if text.len() < 200 { "poor" } else { "good" };
    Ok((text, quality))
}

async fn extract_pptx(path: &str) -> anyhow::Result<(String, &'static str)> {
    let script = format!(
        r###"
from pptx import Presentation
import sys
prs = Presentation(r'{}')
lines = []
for i, slide in enumerate(prs.slides, 1):
    lines.append(f"## Slide {{i}}")
    for shape in slide.shapes:
        if hasattr(shape, "text") and shape.text.strip():
            lines.append(shape.text.strip())
    lines.append("")
print("\n".join(lines))
"###,
        path.replace('\\', "/")
    );

    let output = tokio::process::Command::new("python")
        .arg("-c")
        .arg(&script)
        .output()
        .await
        .context("Failed to spawn python — is python-pptx installed?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("python-pptx extraction failed: {}", stderr);
    }

    let text = String::from_utf8_lossy(&output.stdout).to_string();
    let quality = if text.len() < 100 { "poor" } else { "good" };
    Ok((text, quality))
}

fn cleanup_extracted_text(text: &str) -> String {
    // Collapse 3+ consecutive blank lines to 2
    let mut result = String::new();
    let mut blank_count = 0;
    for line in text.lines() {
        if line.trim().is_empty() {
            blank_count += 1;
            if blank_count <= 2 {
                result.push('\n');
            }
        } else {
            blank_count = 0;
            result.push_str(line);
            result.push('\n');
        }
    }
    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_extracted_text() {
        let input = "line1\n\n\n\n\nline2\n\nline3";
        let out = cleanup_extracted_text(input);
        assert_eq!(out, "line1\n\n\nline2\n\nline3");
    }

    #[test]
    fn test_name() {
        let tool = DevkitDocumentConvertTool;
        assert_eq!(tool.name(), "devkit_document_convert");
    }
}
