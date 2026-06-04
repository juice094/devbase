// SPDX-License-Identifier: MIT
// Copyright (c) 2026 juice094
use std::collections::{HashMap, HashSet};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

pub use tools::*;

/// Phase of a streaming tool invocation.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamPhase {
    /// Progress update (e.g., "Indexing repo 3/10...").
    Progress,
    /// Intermediate partial result.
    Partial,
    /// Final result — stream ends after this.
    Done,
    /// Error occurred — stream ends after this.
    Error,
}

/// A single event in a streaming tool invocation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ToolStreamEvent {
    pub phase: StreamPhase,
    pub payload: serde_json::Value,
}

#[allow(async_fn_in_trait)]
pub trait McpTool: Send + Sync + Clone {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value>;

    /// Optional streaming interface for long-running operations.
    ///
    /// Default implementation delegates to `invoke` and emits a single `Done` event.
    /// Override this for tools that support progressive output (e.g., indexing,
    /// syncing large batches, or long-running analysis).
    async fn invoke_stream(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<Vec<ToolStreamEvent>> {
        let result = self.invoke(args, ctx).await?;
        Ok(vec![ToolStreamEvent {
            phase: StreamPhase::Done,
            payload: result,
        }])
    }
}

#[derive(Clone)]
pub enum McpToolEnum {
    Scan(DevkitScanTool),
    Health(DevkitHealthTool),
    Sync(DevkitSyncTool),
    Query(DevkitQueryTool),
    QueryRepos(DevkitQueryReposTool),
    Index(DevkitIndexTool),
    IndexHealth(DevkitIndexHealthTool),
    IndexStream(DevkitIndexStreamTool),
    Note(DevkitNoteTool),
    Status(DevkitStatusTool),
    Digest(DevkitDigestTool),
    Paper(DevkitPaperIndexTool),
    Experiment(DevkitExperimentLogTool),
    GithubInfo(DevkitGithubInfoTool),
    CodeMetrics(DevkitCodeMetricsTool),
    ModuleGraph(DevkitModuleGraphTool),
    NaturalLanguageQuery(DevkitNaturalLanguageQueryTool),
    VaultSearch(DevkitVaultSearchTool),
    VaultRead(DevkitVaultReadTool),
    VaultWrite(DevkitVaultWriteTool),
    VaultBacklinks(DevkitVaultBacklinksTool),
    VaultDaily(DevkitVaultDailyTool),
    VaultGraph(DevkitVaultGraphTool),
    VaultExport(DevkitVaultExportTool),
    VaultHistory(DevkitVaultHistoryTool),
    ProjectContext(DevkitProjectContextTool),
    ProjectBrief(DevkitProjectBriefTool),
    ImpactAnalysis(DevkitImpactAnalysisTool),
    CodeSymbols(DevkitCodeSymbolsTool),
    DependencyGraph(DevkitDependencyGraphTool),
    CallGraph(DevkitCallGraphTool),
    DeadCode(DevkitDeadCodeTool),
    SemanticSearch(DevkitSemanticSearchTool),
    ArxivFetch(DevkitArxivFetchTool),
    EmbeddingStore(DevkitEmbeddingStoreTool),
    EmbeddingSearch(DevkitEmbeddingSearchTool),
    CrossRepoSearch(DevkitCrossRepoSearchTool),
    KnowledgeReport(DevkitKnowledgeReportTool),
    RelatedSymbols(DevkitRelatedSymbolsTool),
    HybridSearch(DevkitHybridSearchTool),
    SearchQuality(DevkitSearchQualityTool),
    SkillList(DevkitSkillListTool),
    SkillSearch(DevkitSkillSearchTool),
    SkillRun(DevkitSkillRunTool),
    SkillDiscover(DevkitSkillDiscoverTool),
    SkillSync(DevkitSkillSyncTool),
    KnownLimitStore(DevkitKnownLimitStoreTool),
    KnownLimitList(DevkitKnownLimitListTool),
    RelationStore(DevkitRelationStoreTool),
    RelationQuery(DevkitRelationQueryTool),
    RelationDelete(DevkitRelationDeleteTool),
    SessionSave(DevkitSessionSaveTool),
    SessionList(DevkitSessionListTool),
    SessionResume(DevkitSessionResumeTool),
    SessionAttach(DevkitSessionAttachTool),
    SessionDetach(DevkitSessionDetachTool),
    SessionActivate(DevkitSessionActivateTool),
    SessionSearch(DevkitSessionSearchTool),
    SessionCapture(DevkitSessionCaptureTool),
    SessionWorkflows(DevkitSessionWorkflowsTool),
    SessionRecall(DevkitSessionRecallTool),
    SessionIndex(DevkitSessionIndexTool),
    SessionExport(DevkitSessionExportTool),
    SessionImport(DevkitSessionImportTool),
    WorkflowList(DevkitWorkflowListTool),
    WorkflowRun(DevkitWorkflowRunTool),
    WorkflowStatus(DevkitWorkflowStatusTool),
    OplogQuery(DevkitOplogQueryTool),
    Evaluate(DevkitEvaluateTool),
    DocumentConvert(DevkitDocumentConvertTool),
}

/// Stability tier for MCP tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTier {
    Stable,
    Beta,
    Experimental,
}

impl std::str::FromStr for ToolTier {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "stable" => Ok(ToolTier::Stable),
            "beta" => Ok(ToolTier::Beta),
            "experimental" => Ok(ToolTier::Experimental),
            _ => Err(()),
        }
    }
}

impl McpToolEnum {
    pub fn tier(&self) -> ToolTier {
        match self {
            // Stable: battle-tested, schema frozen, unit-tested
            McpToolEnum::Health(_) => ToolTier::Stable,
            McpToolEnum::QueryRepos(_) => ToolTier::Stable,
            McpToolEnum::VaultSearch(_) => ToolTier::Stable,
            McpToolEnum::VaultRead(_) => ToolTier::Stable,
            McpToolEnum::ProjectContext(_) => ToolTier::Stable,
            McpToolEnum::ProjectBrief(_) => ToolTier::Beta,
            McpToolEnum::ImpactAnalysis(_) => ToolTier::Beta,
            // Beta: validated but schema may微调, limited edge-case tests
            McpToolEnum::Scan(_) => ToolTier::Beta,
            McpToolEnum::Sync(_) => ToolTier::Beta,
            McpToolEnum::Query(_) => ToolTier::Beta,
            McpToolEnum::Index(_) => ToolTier::Beta,
            McpToolEnum::IndexHealth(_) => ToolTier::Beta,
            McpToolEnum::IndexStream(_) => ToolTier::Beta,
            McpToolEnum::Status(_) => ToolTier::Beta,
            McpToolEnum::Note(_) => ToolTier::Beta,
            McpToolEnum::VaultWrite(_) => ToolTier::Beta,
            McpToolEnum::VaultBacklinks(_) => ToolTier::Beta,
            McpToolEnum::VaultDaily(_) => ToolTier::Beta,
            McpToolEnum::VaultGraph(_) => ToolTier::Beta,
            McpToolEnum::VaultExport(_) => ToolTier::Beta,
            McpToolEnum::VaultHistory(_) => ToolTier::Beta,
            McpToolEnum::NaturalLanguageQuery(_) => ToolTier::Beta,
            McpToolEnum::GithubInfo(_) => ToolTier::Beta,
            // Experimental: new, behavior may change, pending prod validation
            McpToolEnum::Digest(_) => ToolTier::Experimental,
            McpToolEnum::Paper(_) => ToolTier::Experimental,
            McpToolEnum::Experiment(_) => ToolTier::Beta,
            McpToolEnum::CodeMetrics(_) => ToolTier::Beta,
            McpToolEnum::ModuleGraph(_) => ToolTier::Beta,
            McpToolEnum::CodeSymbols(_) => ToolTier::Beta,
            McpToolEnum::DependencyGraph(_) => ToolTier::Beta,
            McpToolEnum::CallGraph(_) => ToolTier::Beta,
            McpToolEnum::DeadCode(_) => ToolTier::Beta,
            McpToolEnum::SemanticSearch(_) => ToolTier::Beta,
            McpToolEnum::ArxivFetch(_) => ToolTier::Beta,
            McpToolEnum::EmbeddingStore(_) => ToolTier::Beta,
            McpToolEnum::EmbeddingSearch(_) => ToolTier::Beta,
            McpToolEnum::CrossRepoSearch(_) => ToolTier::Beta,
            McpToolEnum::KnowledgeReport(_) => ToolTier::Beta,
            McpToolEnum::RelatedSymbols(_) => ToolTier::Experimental,
            McpToolEnum::HybridSearch(_) => ToolTier::Beta,
            McpToolEnum::SearchQuality(_) => ToolTier::Beta,
            McpToolEnum::SkillList(_) => ToolTier::Beta,
            McpToolEnum::SkillSearch(_) => ToolTier::Beta,
            McpToolEnum::SkillRun(_) => ToolTier::Beta,
            McpToolEnum::SkillDiscover(_) => ToolTier::Beta,
            McpToolEnum::SkillSync(_) => ToolTier::Beta,
            McpToolEnum::KnownLimitStore(_) => ToolTier::Beta,
            McpToolEnum::KnownLimitList(_) => ToolTier::Beta,
            McpToolEnum::RelationStore(_) => ToolTier::Beta,
            McpToolEnum::RelationQuery(_) => ToolTier::Beta,
            McpToolEnum::RelationDelete(_) => ToolTier::Beta,
            McpToolEnum::SessionSave(_) => ToolTier::Beta,
            McpToolEnum::SessionList(_) => ToolTier::Beta,
            McpToolEnum::SessionResume(_) => ToolTier::Beta,
            McpToolEnum::SessionAttach(_) => ToolTier::Beta,
            McpToolEnum::SessionDetach(_) => ToolTier::Beta,
            McpToolEnum::SessionActivate(_) => ToolTier::Beta,
            McpToolEnum::SessionSearch(_) => ToolTier::Beta,
            McpToolEnum::SessionCapture(_) => ToolTier::Beta,
            McpToolEnum::SessionWorkflows(_) => ToolTier::Beta,
            McpToolEnum::SessionRecall(_) => ToolTier::Experimental,
            McpToolEnum::SessionIndex(_) => ToolTier::Experimental,
            McpToolEnum::SessionExport(_) => ToolTier::Experimental,
            McpToolEnum::SessionImport(_) => ToolTier::Experimental,
            McpToolEnum::WorkflowList(_) => ToolTier::Beta,
            McpToolEnum::WorkflowRun(_) => ToolTier::Beta,
            McpToolEnum::WorkflowStatus(_) => ToolTier::Beta,
            McpToolEnum::OplogQuery(_) => ToolTier::Beta,
            McpToolEnum::Evaluate(_) => ToolTier::Beta,
            McpToolEnum::DocumentConvert(_) => ToolTier::Experimental,
        }
    }
}

impl McpTool for McpToolEnum {
    fn name(&self) -> &'static str {
        match self {
            McpToolEnum::Scan(t) => t.name(),
            McpToolEnum::Health(t) => t.name(),
            McpToolEnum::Sync(t) => t.name(),
            McpToolEnum::Query(t) => t.name(),
            McpToolEnum::QueryRepos(t) => t.name(),
            McpToolEnum::Index(t) => t.name(),
            McpToolEnum::IndexHealth(t) => t.name(),
            McpToolEnum::IndexStream(t) => t.name(),
            McpToolEnum::Status(t) => t.name(),
            McpToolEnum::Note(t) => t.name(),
            McpToolEnum::Digest(t) => t.name(),
            McpToolEnum::Paper(t) => t.name(),
            McpToolEnum::Experiment(t) => t.name(),
            McpToolEnum::GithubInfo(t) => t.name(),
            McpToolEnum::CodeMetrics(t) => t.name(),
            McpToolEnum::ModuleGraph(t) => t.name(),
            McpToolEnum::NaturalLanguageQuery(t) => t.name(),
            McpToolEnum::VaultSearch(t) => t.name(),
            McpToolEnum::VaultRead(t) => t.name(),
            McpToolEnum::VaultWrite(t) => t.name(),
            McpToolEnum::VaultBacklinks(t) => t.name(),
            McpToolEnum::VaultDaily(t) => t.name(),
            McpToolEnum::VaultGraph(t) => t.name(),
            McpToolEnum::VaultExport(t) => t.name(),
            McpToolEnum::VaultHistory(t) => t.name(),
            McpToolEnum::ProjectContext(t) => t.name(),
            McpToolEnum::ProjectBrief(t) => t.name(),
            McpToolEnum::ImpactAnalysis(t) => t.name(),
            McpToolEnum::CodeSymbols(t) => t.name(),
            McpToolEnum::DependencyGraph(t) => t.name(),
            McpToolEnum::CallGraph(t) => t.name(),
            McpToolEnum::DeadCode(t) => t.name(),
            McpToolEnum::SemanticSearch(t) => t.name(),
            McpToolEnum::ArxivFetch(t) => t.name(),
            McpToolEnum::EmbeddingStore(t) => t.name(),
            McpToolEnum::EmbeddingSearch(t) => t.name(),
            McpToolEnum::CrossRepoSearch(t) => t.name(),
            McpToolEnum::KnowledgeReport(t) => t.name(),
            McpToolEnum::RelatedSymbols(t) => t.name(),
            McpToolEnum::HybridSearch(t) => t.name(),
            McpToolEnum::SearchQuality(t) => t.name(),
            McpToolEnum::SkillList(t) => t.name(),
            McpToolEnum::SkillSearch(t) => t.name(),
            McpToolEnum::SkillRun(t) => t.name(),
            McpToolEnum::SkillDiscover(t) => t.name(),
            McpToolEnum::SkillSync(t) => t.name(),
            McpToolEnum::KnownLimitStore(t) => t.name(),
            McpToolEnum::KnownLimitList(t) => t.name(),
            McpToolEnum::RelationStore(t) => t.name(),
            McpToolEnum::RelationQuery(t) => t.name(),
            McpToolEnum::RelationDelete(t) => t.name(),
            McpToolEnum::SessionSave(t) => t.name(),
            McpToolEnum::SessionList(t) => t.name(),
            McpToolEnum::SessionResume(t) => t.name(),
            McpToolEnum::SessionAttach(t) => t.name(),
            McpToolEnum::SessionDetach(t) => t.name(),
            McpToolEnum::SessionActivate(t) => t.name(),
            McpToolEnum::SessionSearch(t) => t.name(),
            McpToolEnum::SessionCapture(t) => t.name(),
            McpToolEnum::SessionWorkflows(t) => t.name(),
            McpToolEnum::SessionRecall(t) => t.name(),
            McpToolEnum::SessionIndex(t) => t.name(),
            McpToolEnum::SessionExport(t) => t.name(),
            McpToolEnum::SessionImport(t) => t.name(),
            McpToolEnum::WorkflowList(t) => t.name(),
            McpToolEnum::WorkflowRun(t) => t.name(),
            McpToolEnum::WorkflowStatus(t) => t.name(),
            McpToolEnum::OplogQuery(t) => t.name(),
            McpToolEnum::Evaluate(t) => t.name(),
            McpToolEnum::DocumentConvert(t) => t.name(),
        }
    }

    fn schema(&self) -> serde_json::Value {
        match self {
            McpToolEnum::Scan(t) => t.schema(),
            McpToolEnum::Health(t) => t.schema(),
            McpToolEnum::Sync(t) => t.schema(),
            McpToolEnum::Query(t) => t.schema(),
            McpToolEnum::QueryRepos(t) => t.schema(),
            McpToolEnum::Index(t) => t.schema(),
            McpToolEnum::IndexHealth(t) => t.schema(),
            McpToolEnum::IndexStream(t) => t.schema(),
            McpToolEnum::Status(t) => t.schema(),
            McpToolEnum::Note(t) => t.schema(),
            McpToolEnum::Digest(t) => t.schema(),
            McpToolEnum::Paper(t) => t.schema(),
            McpToolEnum::Experiment(t) => t.schema(),
            McpToolEnum::GithubInfo(t) => t.schema(),
            McpToolEnum::CodeMetrics(t) => t.schema(),
            McpToolEnum::ModuleGraph(t) => t.schema(),
            McpToolEnum::NaturalLanguageQuery(t) => t.schema(),
            McpToolEnum::VaultSearch(t) => t.schema(),
            McpToolEnum::VaultRead(t) => t.schema(),
            McpToolEnum::VaultWrite(t) => t.schema(),
            McpToolEnum::VaultBacklinks(t) => t.schema(),
            McpToolEnum::VaultDaily(t) => t.schema(),
            McpToolEnum::VaultGraph(t) => t.schema(),
            McpToolEnum::VaultExport(t) => t.schema(),
            McpToolEnum::VaultHistory(t) => t.schema(),
            McpToolEnum::ProjectContext(t) => t.schema(),
            McpToolEnum::ProjectBrief(t) => t.schema(),
            McpToolEnum::ImpactAnalysis(t) => t.schema(),
            McpToolEnum::CodeSymbols(t) => t.schema(),
            McpToolEnum::DependencyGraph(t) => t.schema(),
            McpToolEnum::CallGraph(t) => t.schema(),
            McpToolEnum::DeadCode(t) => t.schema(),
            McpToolEnum::SemanticSearch(t) => t.schema(),
            McpToolEnum::ArxivFetch(t) => t.schema(),
            McpToolEnum::EmbeddingStore(t) => t.schema(),
            McpToolEnum::EmbeddingSearch(t) => t.schema(),
            McpToolEnum::CrossRepoSearch(t) => t.schema(),
            McpToolEnum::KnowledgeReport(t) => t.schema(),
            McpToolEnum::RelatedSymbols(t) => t.schema(),
            McpToolEnum::HybridSearch(t) => t.schema(),
            McpToolEnum::SearchQuality(t) => t.schema(),
            McpToolEnum::SkillList(t) => t.schema(),
            McpToolEnum::SkillSearch(t) => t.schema(),
            McpToolEnum::SkillRun(t) => t.schema(),
            McpToolEnum::SkillDiscover(t) => t.schema(),
            McpToolEnum::SkillSync(t) => t.schema(),
            McpToolEnum::KnownLimitStore(t) => t.schema(),
            McpToolEnum::KnownLimitList(t) => t.schema(),
            McpToolEnum::RelationStore(t) => t.schema(),
            McpToolEnum::RelationQuery(t) => t.schema(),
            McpToolEnum::RelationDelete(t) => t.schema(),
            McpToolEnum::SessionSave(t) => t.schema(),
            McpToolEnum::SessionList(t) => t.schema(),
            McpToolEnum::SessionResume(t) => t.schema(),
            McpToolEnum::SessionAttach(t) => t.schema(),
            McpToolEnum::SessionDetach(t) => t.schema(),
            McpToolEnum::SessionActivate(t) => t.schema(),
            McpToolEnum::SessionSearch(t) => t.schema(),
            McpToolEnum::SessionCapture(t) => t.schema(),
            McpToolEnum::SessionWorkflows(t) => t.schema(),
            McpToolEnum::SessionRecall(t) => t.schema(),
            McpToolEnum::SessionIndex(t) => t.schema(),
            McpToolEnum::SessionExport(t) => t.schema(),
            McpToolEnum::SessionImport(t) => t.schema(),
            McpToolEnum::WorkflowList(t) => t.schema(),
            McpToolEnum::WorkflowRun(t) => t.schema(),
            McpToolEnum::WorkflowStatus(t) => t.schema(),
            McpToolEnum::OplogQuery(t) => t.schema(),
            McpToolEnum::Evaluate(t) => t.schema(),
            McpToolEnum::DocumentConvert(t) => t.schema(),
        }
    }

    async fn invoke(
        &self,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        match self {
            McpToolEnum::Scan(t) => t.invoke(args, ctx).await,
            McpToolEnum::Health(t) => t.invoke(args, ctx).await,
            McpToolEnum::Sync(t) => t.invoke(args, ctx).await,
            McpToolEnum::Query(t) => t.invoke(args, ctx).await,
            McpToolEnum::QueryRepos(t) => t.invoke(args, ctx).await,
            McpToolEnum::Index(t) => t.invoke(args, ctx).await,
            McpToolEnum::IndexHealth(t) => t.invoke(args, ctx).await,
            McpToolEnum::IndexStream(t) => t.invoke(args, ctx).await,
            McpToolEnum::Status(t) => t.invoke(args, ctx).await,
            McpToolEnum::Note(t) => t.invoke(args, ctx).await,
            McpToolEnum::Digest(t) => t.invoke(args, ctx).await,
            McpToolEnum::Paper(t) => t.invoke(args, ctx).await,
            McpToolEnum::Experiment(t) => t.invoke(args, ctx).await,
            McpToolEnum::GithubInfo(t) => t.invoke(args, ctx).await,
            McpToolEnum::CodeMetrics(t) => t.invoke(args, ctx).await,
            McpToolEnum::ModuleGraph(t) => t.invoke(args, ctx).await,
            McpToolEnum::NaturalLanguageQuery(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultSearch(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultRead(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultWrite(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultBacklinks(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultDaily(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultGraph(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultExport(t) => t.invoke(args, ctx).await,
            McpToolEnum::VaultHistory(t) => t.invoke(args, ctx).await,
            McpToolEnum::ProjectContext(t) => t.invoke(args, ctx).await,
            McpToolEnum::ProjectBrief(t) => t.invoke(args, ctx).await,
            McpToolEnum::ImpactAnalysis(t) => t.invoke(args, ctx).await,
            McpToolEnum::CodeSymbols(t) => t.invoke(args, ctx).await,
            McpToolEnum::DependencyGraph(t) => t.invoke(args, ctx).await,
            McpToolEnum::CallGraph(t) => t.invoke(args, ctx).await,
            McpToolEnum::DeadCode(t) => t.invoke(args, ctx).await,
            McpToolEnum::SemanticSearch(t) => t.invoke(args, ctx).await,
            McpToolEnum::ArxivFetch(t) => t.invoke(args, ctx).await,
            McpToolEnum::EmbeddingStore(t) => t.invoke(args, ctx).await,
            McpToolEnum::EmbeddingSearch(t) => t.invoke(args, ctx).await,
            McpToolEnum::CrossRepoSearch(t) => t.invoke(args, ctx).await,
            McpToolEnum::KnowledgeReport(t) => t.invoke(args, ctx).await,
            McpToolEnum::RelatedSymbols(t) => t.invoke(args, ctx).await,
            McpToolEnum::HybridSearch(t) => t.invoke(args, ctx).await,
            McpToolEnum::SearchQuality(t) => t.invoke(args, ctx).await,
            McpToolEnum::SkillList(t) => t.invoke(args, ctx).await,
            McpToolEnum::SkillSearch(t) => t.invoke(args, ctx).await,
            McpToolEnum::SkillRun(t) => t.invoke(args, ctx).await,
            McpToolEnum::SkillDiscover(t) => t.invoke(args, ctx).await,
            McpToolEnum::SkillSync(t) => t.invoke(args, ctx).await,
            McpToolEnum::KnownLimitStore(t) => t.invoke(args, ctx).await,
            McpToolEnum::KnownLimitList(t) => t.invoke(args, ctx).await,
            McpToolEnum::RelationStore(t) => t.invoke(args, ctx).await,
            McpToolEnum::RelationQuery(t) => t.invoke(args, ctx).await,
            McpToolEnum::RelationDelete(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionSave(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionList(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionResume(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionAttach(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionDetach(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionActivate(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionSearch(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionCapture(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionWorkflows(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionRecall(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionIndex(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionExport(t) => t.invoke(args, ctx).await,
            McpToolEnum::SessionImport(t) => t.invoke(args, ctx).await,
            McpToolEnum::WorkflowList(t) => t.invoke(args, ctx).await,
            McpToolEnum::WorkflowRun(t) => t.invoke(args, ctx).await,
            McpToolEnum::WorkflowStatus(t) => t.invoke(args, ctx).await,
            McpToolEnum::OplogQuery(t) => t.invoke(args, ctx).await,
            McpToolEnum::Evaluate(t) => t.invoke(args, ctx).await,
            McpToolEnum::DocumentConvert(t) => t.invoke(args, ctx).await,
        }
    }
}

/// Long-lived oplog file handle — opened once, reused across all MCP calls.
static OPLOG_FILE: std::sync::OnceLock<std::sync::Mutex<Option<std::fs::File>>> =
    std::sync::OnceLock::new();

fn get_oplog_file() -> &'static std::sync::Mutex<Option<std::fs::File>> {
    OPLOG_FILE.get_or_init(|| {
        let file = dirs::data_local_dir().and_then(|data_dir| {
            let log_path = data_dir.join("devbase").join("mcp-oplog.ndjson");
            std::fs::OpenOptions::new().create(true).append(true).open(&log_path).ok()
        });
        std::sync::Mutex::new(file)
    })
}

/// Append a single MCP tool invocation record to the oplog file.
///
/// Path: `%LOCALAPPDATA%/devbase/mcp-oplog.ndjson`
/// Format: newline-delimited JSON (NDJSON)
fn append_mcp_oplog(tool_name: &str, duration_ms: u128, success: bool, error_type: Option<&str>) {
    let entry = serde_json::json!({
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "tool": tool_name,
        "duration_ms": duration_ms,
        "success": success,
        "error_type": error_type,
    });

    if let Ok(mut guard) = get_oplog_file().lock() {
        if let Some(ref mut file) = *guard {
            use std::io::Write;
            if let Err(e) = writeln!(file, "{}", entry) {
                tracing::warn!("Failed to write MCP oplog: {}", e);
            }
        }
    }
}

pub struct McpServer {
    tools: HashMap<String, McpToolEnum>,
}

impl Default for McpServer {
    fn default() -> Self {
        Self::new()
    }
}

impl McpServer {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register_tool(&mut self, tool: McpToolEnum) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    pub async fn handle_request(
        &self,
        req: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<serde_json::Value> {
        let id = req.get("id").cloned().unwrap_or(serde_json::Value::Null);
        let method = req.get("method").and_then(|v| v.as_str()).unwrap_or("");

        match method {
            "ping" => Ok(serde_json::json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {}
            })),
            "initialize" => {
                // Verify client protocol version for compatibility
                let client_version = req
                    .get("params")
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                let supported = ["2024-11-05"];
                if !supported.contains(&client_version) {
                    tracing::warn!(
                        "Client protocol version '{}' not in supported list {:?}; proceeding with 2024-11-05",
                        client_version,
                        supported
                    );
                }
                Ok(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": {
                        "protocolVersion": "2024-11-05",
                        "capabilities": {
                            "tools": {}
                        },
                        "serverInfo": {
                            "name": "devbase",
                            "version": env!("CARGO_PKG_VERSION")
                        }
                    }
                }))
            }
            "tools/list" => {
                let tools: Vec<serde_json::Value> = self
                    .tools
                    .values()
                    .map(|t| {
                        let schema = t.schema();
                        serde_json::json!({
                            "name": t.name(),
                            "description": schema.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                            "inputSchema": schema.get("inputSchema").cloned().unwrap_or(serde_json::json!({}))
                        })
                    })
                    .collect();
                Ok(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "result": { "tools": tools }
                }))
            }
            "tools/call" => {
                let params = req.get("params").cloned().unwrap_or(serde_json::Value::Null);
                let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(serde_json::Value::Null);
                let stream = params.get("stream").and_then(|v| v.as_bool()).unwrap_or(false);

                match self.tools.get(name) {
                    Some(_tool) if stream => {
                        let start = std::time::Instant::now();
                        match self.handle_streaming_call(name, args, ctx).await {
                            Ok(events) => {
                                append_mcp_oplog(name, start.elapsed().as_millis(), true, None);
                                let events_json = serde_json::to_string(&events)?;
                                let content = serde_json::json!({
                                    "type": "text",
                                    "text": events_json
                                });
                                Ok(serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "content": [content],
                                        "isError": false
                                    }
                                }))
                            }
                            Err(e) => {
                                append_mcp_oplog(
                                    name,
                                    start.elapsed().as_millis(),
                                    false,
                                    Some("invoke_error"),
                                );
                                let payload =
                                    serde_json::json!({ "success": false, "error": e.to_string() });
                                let text = serde_json::to_string(&payload)?;
                                let content = serde_json::json!({ "type": "text", "text": text });
                                Ok(serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "content": [content],
                                        "isError": true
                                    }
                                }))
                            }
                        }
                    }
                    Some(tool) => {
                        let start = std::time::Instant::now();
                        match tool.invoke(args, ctx).await {
                            Ok(result) => {
                                let text = result.to_string();
                                let is_error = !result
                                    .get("success")
                                    .and_then(|v: &serde_json::Value| v.as_bool())
                                    .unwrap_or(true);
                                append_mcp_oplog(
                                    name,
                                    start.elapsed().as_millis(),
                                    !is_error,
                                    None,
                                );
                                let content = serde_json::json!({
                                    "type": "text",
                                    "text": text
                                });
                                Ok(serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "content": [content],
                                        "isError": is_error
                                    }
                                }))
                            }
                            Err(e) => {
                                append_mcp_oplog(
                                    name,
                                    start.elapsed().as_millis(),
                                    false,
                                    Some("invoke_error"),
                                );
                                let payload =
                                    serde_json::json!({ "success": false, "error": e.to_string() });
                                let text = serde_json::to_string(&payload)?;
                                let content = serde_json::json!({ "type": "text", "text": text });
                                Ok(serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": id,
                                    "result": {
                                        "content": [content],
                                        "isError": true
                                    }
                                }))
                            }
                        }
                    }
                    None => Ok(serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": id,
                        "error": {
                            "code": -32602,
                            "message": format!("Tool '{}' not found", name)
                        }
                    })),
                }
            }
            _ => {
                if id.is_null() {
                    // Workaround: Python MCP SDK 1.16.0 cannot parse JSON-RPC
                    // error responses with `id: null`. Return Null so the
                    // caller can silently drop it.
                    return Ok(serde_json::Value::Null);
                }
                Ok(serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": id,
                    "error": {
                        "code": -32601,
                        "message": format!("Method '{}' not found", method)
                    }
                }))
            }
        }
    }

    /// Invoke a tool in streaming mode and return a sequence of events.
    ///
    /// This is used by the SSE transport to push progressive updates.
    /// If the tool does not override `invoke_stream`, the default implementation
    /// delegates to `invoke` and wraps the result as a single `Done` event.
    pub async fn handle_streaming_call(
        &self,
        name: &str,
        args: serde_json::Value,
        ctx: &mut crate::storage::AppContext,
    ) -> anyhow::Result<Vec<ToolStreamEvent>> {
        match self.tools.get(name) {
            Some(tool) => tool.invoke_stream(args, ctx).await,
            None => Err(anyhow::anyhow!("Tool '{}' not found", name)),
        }
    }
}

/// Build an MCP server with optional tier filtering.
///
/// If `tiers` is `None`, all 69 tools are registered (backward compatible).
/// If `tiers` is provided, only tools whose tier is in the set are registered.
pub fn build_server_with_tiers(tiers: Option<&HashSet<ToolTier>>) -> McpServer {
    let mut server = McpServer::new();
    let all_tools = [
        McpToolEnum::Scan(DevkitScanTool),
        McpToolEnum::Health(DevkitHealthTool),
        McpToolEnum::Sync(DevkitSyncTool),
        McpToolEnum::Query(DevkitQueryTool),
        McpToolEnum::QueryRepos(DevkitQueryReposTool),
        McpToolEnum::Index(DevkitIndexTool),
        McpToolEnum::IndexHealth(DevkitIndexHealthTool),
        McpToolEnum::IndexStream(DevkitIndexStreamTool),
        McpToolEnum::Status(DevkitStatusTool),
        McpToolEnum::Note(DevkitNoteTool),
        McpToolEnum::Digest(DevkitDigestTool),
        McpToolEnum::Paper(DevkitPaperIndexTool),
        McpToolEnum::Experiment(DevkitExperimentLogTool),
        McpToolEnum::GithubInfo(DevkitGithubInfoTool),
        McpToolEnum::CodeMetrics(DevkitCodeMetricsTool),
        McpToolEnum::ModuleGraph(DevkitModuleGraphTool),
        McpToolEnum::NaturalLanguageQuery(DevkitNaturalLanguageQueryTool),
        McpToolEnum::VaultSearch(DevkitVaultSearchTool),
        McpToolEnum::VaultRead(DevkitVaultReadTool),
        McpToolEnum::VaultWrite(DevkitVaultWriteTool),
        McpToolEnum::VaultBacklinks(DevkitVaultBacklinksTool),
        McpToolEnum::VaultDaily(DevkitVaultDailyTool),
        McpToolEnum::VaultGraph(DevkitVaultGraphTool),
        McpToolEnum::VaultExport(DevkitVaultExportTool),
        McpToolEnum::VaultHistory(DevkitVaultHistoryTool),
        McpToolEnum::ProjectContext(DevkitProjectContextTool),
        McpToolEnum::ProjectBrief(DevkitProjectBriefTool),
        McpToolEnum::ImpactAnalysis(DevkitImpactAnalysisTool),
        McpToolEnum::CodeSymbols(DevkitCodeSymbolsTool),
        McpToolEnum::DependencyGraph(DevkitDependencyGraphTool),
        McpToolEnum::CallGraph(DevkitCallGraphTool),
        McpToolEnum::DeadCode(DevkitDeadCodeTool),
        McpToolEnum::SemanticSearch(DevkitSemanticSearchTool),
        McpToolEnum::ArxivFetch(DevkitArxivFetchTool),
        McpToolEnum::EmbeddingStore(DevkitEmbeddingStoreTool),
        McpToolEnum::EmbeddingSearch(DevkitEmbeddingSearchTool),
        McpToolEnum::CrossRepoSearch(DevkitCrossRepoSearchTool),
        McpToolEnum::KnowledgeReport(DevkitKnowledgeReportTool),
        McpToolEnum::RelatedSymbols(DevkitRelatedSymbolsTool),
        McpToolEnum::HybridSearch(DevkitHybridSearchTool),
        McpToolEnum::SearchQuality(DevkitSearchQualityTool),
        McpToolEnum::SkillList(DevkitSkillListTool),
        McpToolEnum::SkillSearch(DevkitSkillSearchTool),
        McpToolEnum::SkillRun(DevkitSkillRunTool),
        McpToolEnum::SkillDiscover(DevkitSkillDiscoverTool),
        McpToolEnum::SkillSync(DevkitSkillSyncTool),
        McpToolEnum::KnownLimitStore(DevkitKnownLimitStoreTool),
        McpToolEnum::KnownLimitList(DevkitKnownLimitListTool),
        McpToolEnum::RelationStore(DevkitRelationStoreTool),
        McpToolEnum::RelationQuery(DevkitRelationQueryTool),
        McpToolEnum::RelationDelete(DevkitRelationDeleteTool),
        McpToolEnum::SessionSave(DevkitSessionSaveTool),
        McpToolEnum::SessionList(DevkitSessionListTool),
        McpToolEnum::SessionResume(DevkitSessionResumeTool),
        McpToolEnum::SessionAttach(DevkitSessionAttachTool),
        McpToolEnum::SessionDetach(DevkitSessionDetachTool),
        McpToolEnum::SessionActivate(DevkitSessionActivateTool),
        McpToolEnum::SessionSearch(DevkitSessionSearchTool),
        McpToolEnum::SessionCapture(DevkitSessionCaptureTool),
        McpToolEnum::SessionWorkflows(DevkitSessionWorkflowsTool),
        McpToolEnum::SessionRecall(DevkitSessionRecallTool),
        McpToolEnum::SessionIndex(DevkitSessionIndexTool),
        McpToolEnum::SessionExport(DevkitSessionExportTool),
        McpToolEnum::SessionImport(DevkitSessionImportTool),
        McpToolEnum::WorkflowList(DevkitWorkflowListTool),
        McpToolEnum::WorkflowRun(DevkitWorkflowRunTool),
        McpToolEnum::WorkflowStatus(DevkitWorkflowStatusTool),
        McpToolEnum::OplogQuery(DevkitOplogQueryTool),
        McpToolEnum::Evaluate(DevkitEvaluateTool),
        McpToolEnum::DocumentConvert(DevkitDocumentConvertTool),
    ];
    for tool in all_tools {
        if let Some(allowed) = tiers
            && !allowed.contains(&tool.tier())
        {
            continue;
        }
        server.register_tool(tool);
    }
    server
}

/// Build an MCP server with all tools (backward compatible).
pub fn build_server() -> McpServer {
    build_server_with_tiers(None)
}

pub fn format_mcp_message(body: &serde_json::Value) -> String {
    format_mcp_message_auto(body, false)
}

/// Format MCP message with optional NDJSON mode (no Content-Length headers).
/// NDJSON mode outputs raw JSON followed by a newline, for clients that
/// expect line-delimited JSON-RPC over stdio.
pub fn format_mcp_message_auto(body: &serde_json::Value, ndjson: bool) -> String {
    let body_str = body.to_string();
    if ndjson {
        format!("{}\n", body_str)
    } else {
        format!("Content-Length: {}\r\n\r\n{}", body_str.len(), body_str)
    }
}

/// Check whether destructive MCP tools are enabled via environment variable.
/// Returns Ok(()) if enabled, or an error with a clear message if disabled.
pub(crate) fn check_destructive_enabled() -> anyhow::Result<()> {
    let enabled = std::env::var("DEVBASE_MCP_ENABLE_DESTRUCTIVE")
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if !enabled {
        anyhow::bail!(
            "Destructive tools are disabled. \
             Set DEVBASE_MCP_ENABLE_DESTRUCTIVE=1 to enable."
        );
    }
    Ok(())
}

/// Parse tool tiers from a comma-separated string (e.g. "stable,beta").
fn parse_tool_tiers(s: &str) -> HashSet<ToolTier> {
    s.split(',')
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .filter_map(|s| s.parse().ok())
        .collect()
}

pub async fn run_stdio() -> anyhow::Result<()> {
    let mut ctx = crate::storage::AppContext::with_defaults()?;
    let tiers: Option<HashSet<ToolTier>> = std::env::var("DEVBASE_MCP_TOOL_TIERS")
        .ok()
        .map(|s| parse_tool_tiers(&s))
        .filter(|set| !set.is_empty());
    let server = build_server_with_tiers(tiers.as_ref());
    let stdin = tokio::io::stdin();
    let mut stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin);
    let mut line_buf = String::new();
    let mut use_ndjson = false;

    loop {
        line_buf.clear();
        // Read header line to get Content-Length
        let n = reader.read_line(&mut line_buf).await?;
        if n == 0 {
            break; // EOF
        }
        let line = line_buf.trim();
        if line.is_empty() {
            continue;
        }

        let content_length = if line.starts_with("Content-Length: ") {
            line.strip_prefix("Content-Length: ").and_then(|v| v.parse::<usize>().ok())
        } else {
            // Client is using NDJSON (raw JSON lines). Switch to NDJSON output.
            use_ndjson = true;
            // Fallback: parse raw JSON line for backward compatibility
            let req: serde_json::Value = match serde_json::from_str(line) {
                Ok(v) => v,
                Err(e) => {
                    let resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {}", e)
                        }
                    });
                    let msg = format_mcp_message_auto(&resp, use_ndjson);
                    if stdout.write_all(msg.as_bytes()).await.is_err()
                        || stdout.flush().await.is_err()
                    {
                        break;
                    }
                    continue;
                }
            };
            let resp = server.handle_request(req, &mut ctx).await.unwrap_or_else(|e| {
                serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32603,
                        "message": format!("Internal error: {}", e)
                    }
                })
            });
            if !resp.is_null() {
                let msg = format_mcp_message_auto(&resp, use_ndjson);
                if stdout.write_all(msg.as_bytes()).await.is_err() || stdout.flush().await.is_err()
                {
                    break;
                }
            }
            continue;
        };

        let content_length = match content_length {
            Some(len) => len,
            None => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Invalid Content-Length header: {}", line)
                    }
                });
                let msg = format_mcp_message_auto(&resp, use_ndjson);
                if stdout.write_all(msg.as_bytes()).await.is_err() || stdout.flush().await.is_err()
                {
                    break;
                }
                continue;
            }
        };

        // Read the empty line (\r\n or \n)
        line_buf.clear();
        let _ = reader.read_line(&mut line_buf).await;

        // Read the exact number of bytes
        let mut body_buf = vec![0u8; content_length];
        if let Err(e) = reader.read_exact(&mut body_buf).await {
            let resp = serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32700,
                    "message": format!("Failed to read request body: {}", e)
                }
            });
            let msg = format_mcp_message_auto(&resp, use_ndjson);
            if stdout.write_all(msg.as_bytes()).await.is_err() || stdout.flush().await.is_err() {
                break;
            }
            continue;
        }

        let req: serde_json::Value = match String::from_utf8(body_buf) {
            Ok(body) => match serde_json::from_str(&body) {
                Ok(v) => v,
                Err(e) => {
                    let resp = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": null,
                        "error": {
                            "code": -32700,
                            "message": format!("Parse error: {}", e)
                        }
                    });
                    let msg = format_mcp_message_auto(&resp, use_ndjson);
                    if stdout.write_all(msg.as_bytes()).await.is_err()
                        || stdout.flush().await.is_err()
                    {
                        break; // broken pipe
                    }
                    continue;
                }
            },
            Err(e) => {
                let resp = serde_json::json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {
                        "code": -32700,
                        "message": format!("Invalid UTF-8: {}", e)
                    }
                });
                let msg = format_mcp_message_auto(&resp, use_ndjson);
                if stdout.write_all(msg.as_bytes()).await.is_err() || stdout.flush().await.is_err()
                {
                    break; // broken pipe
                }
                continue;
            }
        };

        // Notifications have no "id" field and require no response.
        let is_notification = req.get("id").is_none();
        if is_notification {
            // Silently acknowledge all notifications (not just notifications/*).
            continue;
        }

        let resp = server.handle_request(req, &mut ctx).await.unwrap_or_else(|e| {
            serde_json::json!({
                "jsonrpc": "2.0",
                "id": null,
                "error": {
                    "code": -32603,
                    "message": format!("Internal error: {}", e)
                }
            })
        });

        if !resp.is_null() {
            let msg = format_mcp_message_auto(&resp, use_ndjson);
            if stdout.write_all(msg.as_bytes()).await.is_err() || stdout.flush().await.is_err() {
                break; // broken pipe
            }
        }
    }

    Ok(())
}

pub use crate::clients::*;
#[cfg(test)]
pub mod tests;
pub mod tools;
