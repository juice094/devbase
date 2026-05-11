// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
#![cfg_attr(
    not(any(
        feature = "lang-rust",
        feature = "lang-python",
        feature = "lang-js-ts",
        feature = "lang-go"
    )),
    allow(dead_code, unused_imports, unused_variables)
)]
use std::path::Path;
use tracing::{debug, warn};

use super::{CodeSymbol, Lang, SymbolType};

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

    extract_symbols_from_tree(&tree, file_path, source.as_bytes(), lang)
}

/// Extract symbols from an already-parsed tree-sitter tree.
/// This avoids re-parsing when both symbols and calls are needed.
pub(crate) fn extract_symbols_from_tree(
    tree: &tree_sitter::Tree,
    file_path: &Path,
    source_bytes: &[u8],
    lang: Lang,
) -> Vec<CodeSymbol> {
    let mut symbols = Vec::new();
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
    if let Some(mut sym) = node_to_symbol(node, file_path, source_bytes, lang) {
        sym.attributes = extract_preceding_attributes(node, source_bytes);
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
        #[cfg(feature = "lang-rust")]
        Lang::Rust => rust_node_to_symbol(node, file_path, source_bytes),
        #[cfg(feature = "lang-python")]
        Lang::Python => python_node_to_symbol(node, file_path, source_bytes),
        #[cfg(feature = "lang-js-ts")]
        Lang::JsTs => js_node_to_symbol(node, file_path, source_bytes),
        #[cfg(feature = "lang-go")]
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
        attributes: None,
    })
}

/// Extract `attribute_item` nodes that are immediate preceding siblings of `node`.
/// In tree-sitter-rust 0.24, attributes are siblings, not children, of the item node.
fn extract_preceding_attributes(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    let mut attrs = Vec::new();
    let mut sibling = node.prev_sibling();
    while let Some(sib) = sibling {
        match sib.kind() {
            "attribute_item" => {
                attrs.push(node_text(&sib, source_bytes).to_string());
                sibling = sib.prev_sibling();
            }
            "line_comment" | "block_comment" => {
                // Skip comments that may appear between attributes and the item.
                sibling = sib.prev_sibling();
            }
            _ => break,
        }
    }
    attrs.reverse();
    if attrs.is_empty() {
        None
    } else {
        Some(attrs.join("\n"))
    }
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
        attributes: None,
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
        attributes: None,
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
                attributes: None,
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
                attributes: None,
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
                attributes: None,
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
                attributes: None,
            })
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub(crate) fn extract_node_name(node: &tree_sitter::Node, source_bytes: &[u8]) -> Option<String> {
    for i in 0..node.child_count() {
        let child = node.child(i as u32)?;
        if child.kind() == "identifier" || child.kind() == "type_identifier" {
            return Some(node_text(&child, source_bytes).to_string());
        }
    }
    None
}

pub(crate) fn extract_child_by_kind(
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

pub(crate) fn node_text<'a>(node: &tree_sitter::Node, source: &'a [u8]) -> &'a str {
    std::str::from_utf8(&source[node.start_byte()..node.end_byte()]).unwrap_or("")
}

// ---------------------------------------------------------------------------
// Repository-wide indexing
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "lang-rust")]
    fn test_extract_rust_attributes() {
        let source = r#"
#[tokio::test]
async fn my_test() {
    assert_eq!(1, 1);
}

#[derive(Debug)]
struct MyStruct {
    x: i32,
}
"#;
        let mut parser = tree_sitter::Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into()).unwrap();
        let tree = parser.parse(source, None).unwrap();
        let root = tree.root_node();

        // Find function_item and assert attributes via preceding-sibling extraction
        let func = (0..root.child_count())
            .map(|i| root.child(i as u32).unwrap())
            .find(|n| n.kind() == "function_item")
            .unwrap();
        let attrs = extract_preceding_attributes(&func, source.as_bytes());
        assert!(attrs.is_some(), "Expected attributes to be extracted");
        assert!(
            attrs.as_ref().unwrap().contains("test"),
            "Expected #[tokio::test] in attributes: {:?}",
            attrs
        );

        // Find struct_item and assert derive
        let st = (0..root.child_count())
            .map(|i| root.child(i as u32).unwrap())
            .find(|n| n.kind() == "struct_item")
            .unwrap();
        let attrs = extract_preceding_attributes(&st, source.as_bytes());
        assert!(attrs.is_some(), "Expected derive attribute");
        assert!(
            attrs.as_ref().unwrap().contains("derive"),
            "Expected #[derive(Debug)] in attributes: {:?}",
            attrs
        );
    }
}
