pub mod context;
pub mod known_limit;
pub mod oplog;
pub mod query;
pub mod relations;
pub mod repo;
pub mod skill;
pub mod vault;
pub mod workflow;

pub mod knowledge;
pub mod code_analysis;
pub mod search;
pub mod external;

pub use context::*;
pub use known_limit::*;
pub use oplog::*;
pub use query::*;
pub use relations::*;
pub use repo::*;
pub use skill::*;
pub use vault::*;
pub use workflow::*;

pub use knowledge::*;
pub use code_analysis::*;
pub use search::*;
pub use external::*;

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
        let _ = super::skill::DevkitSkillListTool;
        let _ = super::vault::DevkitVaultSearchTool;
        let _ = super::workflow::DevkitWorkflowListTool;
    }
}
