use std::path::Path;
use tracing::warn;

use super::{CodeCall, Lang};
use super::symbol::{extract_child_by_kind, extract_node_name, node_text};

// ---------------------------------------------------------------------------
// Call extraction
// ---------------------------------------------------------------------------

pub fn extract_calls_from_file(file_path: &Path, source: &str) -> Vec<CodeCall> {
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
