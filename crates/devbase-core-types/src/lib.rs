// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
//! devbase-core-types — Core knowledge graph types: Node, Edge, NodeType.
//!
//! **提取日期**: 2026-05-01 (Workspace split)
//! **零内部耦合**: 此 crate 不依赖 devbase 任何内部模块，仅使用 chrono + std。
//! **职责**: 定义 devbase 统一实体模型的基础类型，替代早期的 repo-centric `RepoEntry` 垄断。
//! **边界**: 纯数据结构，无业务逻辑。所有字段 `pub`，无方法（除 `fm()` 便捷访问器）。
//!
//! 与 devbase 的关系: 被 `registry/`、`vault/`、`search/`、`mcp/tools` 等几乎所有模块引用。
//! 作为 workspace 中最底层的类型 crate，禁止引入任何上层依赖。
//!
//! Design decisions:
//! - NodeType 支持 FromStr: 允许 CLI/MCP 通过字符串灵活指定类型（"repo" / "git" / "vault" / "note"）。
//! - frontmatter 用 HashMap<String, String>: 简单、通用，不绑定 YAML 解析器。
//! - Edge 不含 chrono 字段: 关系本身不需要时间戳，时间戳由存储层（registry）管理。

pub mod edge;
pub mod node;
pub mod node_type;

pub use edge::Edge;
pub use node::Node;
pub use node_type::NodeType;
