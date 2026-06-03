// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
pub mod brief;
pub mod context;
pub mod document_convert;
pub mod evaluate;
pub mod impact;
pub mod index_health;
pub mod known_limit;
pub mod oplog;
pub mod query;
pub mod relations;
pub mod repo;
pub mod session;
pub mod skill;
pub mod status;
pub mod vault;
pub mod workflow;

pub mod code_analysis;
pub mod external;
pub mod knowledge;
pub mod search;

pub use brief::*;
pub use context::*;
pub use document_convert::*;
pub use impact::*;
pub use index_health::*;
pub use known_limit::*;
pub use oplog::*;
pub use query::*;
pub use relations::*;
pub use repo::*;
pub use session::*;
pub use skill::*;
pub use status::*;
pub use vault::*;
pub use workflow::*;

pub use code_analysis::*;
pub use evaluate::*;
pub use external::*;
pub use knowledge::*;
pub use search::*;

#[cfg(test)]
mod tests {
    #[test]
    fn test_tool_modules_compile() {
        // Smoke test: all tool structs are constructible
        let _ = super::context::DevkitProjectContextTool;
        let _ = super::known_limit::DevkitKnownLimitStoreTool;
        let _ = super::oplog::DevkitOplogQueryTool;
        let _ = super::query::DevkitQueryTool;
        let _ = super::repo::DevkitIndexTool;
        let _ = super::status::DevkitStatusTool;
        let _ = super::skill::DevkitSkillListTool;
        let _ = super::vault::DevkitVaultSearchTool;
        let _ = super::vault::DevkitVaultDailyTool;
        let _ = super::vault::DevkitVaultGraphTool;
        let _ = super::workflow::DevkitWorkflowListTool;
        let _ = super::session::DevkitSessionSaveTool;
        let _ = super::session::DevkitSessionListTool;
        let _ = super::session::DevkitSessionResumeTool;
        let _ = super::session::DevkitSessionAttachTool;
        let _ = super::session::DevkitSessionDetachTool;
        let _ = super::session::DevkitSessionActivateTool;
        let _ = super::session::DevkitSessionSearchTool;
        let _ = super::session::DevkitSessionCaptureTool;
        let _ = super::session::DevkitSessionWorkflowsTool;
        let _ = super::session::DevkitSessionRecallTool;
        let _ = super::session::DevkitSessionIndexTool;
        let _ = super::session::DevkitSessionExportTool;
        let _ = super::session::DevkitSessionImportTool;
        let _ = super::brief::DevkitProjectBriefTool;
        let _ = super::impact::DevkitImpactAnalysisTool;
    }
}
