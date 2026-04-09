pub mod tui {
    pub const TITLE_REPOS: &str = "仓库列表";
    pub const TITLE_DETAILS: &str = "详情";
    pub const TITLE_LOGS: &str = "日志";
    pub const TITLE_SYNC_PROGRESS: &str = "同步进度";
    pub const LABEL_ID: &str = "ID: ";
    pub const LABEL_PATH: &str = "路径: ";
    pub const LABEL_BRANCH: &str = "分支: ";
    pub const LABEL_TAGS: &str = "标签: ";
    pub const LABEL_LANGUAGE: &str = "语言: ";
    pub const LABEL_UPSTREAM: &str = "上游: ";
    pub const LABEL_STATUS: &str = "状态: ";
    pub const STATUS_LOADING: &str = "加载中...";
    pub const STATUS_NONE: &str = "无";
    pub const STATUS_NO_UPSTREAM: &str = "无上游仓库";
    pub const STATUS_OK: &str = "正常";
    pub const STATUS_DIRTY: &str = "有未提交修改";
    pub const STATUS_UNKNOWN: &str = "未知";
    pub const HINT_TAG_INPUT: &str = "  [Enter] 确认  [Esc] 取消";
    pub const HINT_POPUP_CLOSE: &str = "按 Esc 或 Enter 关闭";
    pub const HELP_QUIT: &str = "退出";
    pub const HELP_REFRESH: &str = "刷新";
    pub const HELP_PREVIEW: &str = "获取预览";
    pub const HELP_BATCH: &str = "批量同步";
    pub const HELP_TAG: &str = "编辑标签";
    pub const HELP_HELP: &str = "帮助";
    pub const HELP_NAVIGATE: &str = "导航";
}

pub mod cli {
    pub const SCANNING: &str = "正在扫描目录";
    pub const HEALTH_CHECK: &str = "正在运行健康检查";
    pub const SYNCING: &str = "正在同步";
    pub const QUERYING: &str = "正在查询";
    pub const INDEXING: &str = "正在索引";
    pub const LAUNCHING_TUI: &str = "正在启动 TUI";
    pub const GENERATING_DIGEST: &str = "正在生成日报";
}

pub mod sync {
    pub const STRATEGY_PREFIX: &str = "同步策略";
    pub const FILTER_PREFIX: &str = "（标签过滤: ";
    pub const SKIP_NO_UPSTREAM: &str = "跳过（自有项目，无上游）";
    pub const CHECKING: &str = "检查中";
    pub const ERROR_PREFIX: &str = "错误";
    pub const FETCHED_ONLY: &str = "仅获取。使用 --strategy=auto-pull 或 ask 进行合并。";
    pub const BLOCKED_DIRTY: &str = "阻塞：工作目录不干净。请先提交或暂存。";
    pub const MERGED_FF: &str = "已快进合并到 origin";
    pub const MERGED_COMMIT: &str = "已创建合并提交";
    pub const CONFLICT: &str = "合并存在冲突，请手动解决。";
    pub const DRY_RUN_COMPLETE: &str = "ℹ️  仅预览完成，未应用任何更改。";
    pub const SYNC_COMPLETE: &str = "✅ 同步完成。";
    pub const NO_REPOS_PROCESSED: &str = "没有处理任何仓库。";
    pub const HEADER_REPO: &str = "仓库";
    pub const HEADER_ACTION: &str = "动作";
    pub const HEADER_RESULT: &str = "结果";
    pub const HEADER_ERROR: &str = "错误类型";
    pub const SUMMARY_TOTAL: &str = "总计";
    pub const SUMMARY_SUCCESS: &str = "成功";
    pub const SUMMARY_ERRORS: &str = "错误";
    pub const SUMMARY_SKIPPED: &str = "跳过";
}

pub mod log {
    pub const TUI_STARTED: &str = "devbase TUI 已启动。按 'h' 查看帮助，'q' 退出。";
    pub const NO_REPO_SELECTED: &str = "未选择仓库。";
    pub const REFRESHING: &str = "正在刷新注册表...";
    pub const TAG_INPUT_CANCELLED: &str = "已取消标签输入。";
    pub const EMPTY_TAG_IGNORED: &str = "忽略空标签输入。";
    pub const NO_TAGS_TO_SYNC: &str = "所选仓库没有可用于批量同步的标签。";
    pub const NO_REPOS_MATCH_TAGS: &str = "没有仓库匹配所选标签。";
    pub const SYNC_FINISHED: &str = "同步结束";
    pub const PROGRESS: &str = "进度";
    pub const NO_REPOS_REGISTERED: &str = "没有已注册的仓库。请先运行 'devbase scan <路径> --register'。";

    pub fn loaded_repos(count: usize) -> String {
        format!("已加载 {} 个仓库。", count)
    }
    pub fn fetching_preview(repo_id: &str) -> String {
        format!("正在为 {} 获取预览...", repo_id)
    }
    pub fn preview_done(repo_id: &str, ahead: usize, behind: usize) -> String {
        format!("{} 的预览：ahead={} behind={}", repo_id, ahead, behind)
    }
    pub fn updated_tags(repo_id: &str, tags: &str) -> String {
        format!("已更新 [{}] 的标签为：{}", repo_id, tags)
    }
    pub fn batch_syncing(count: usize) -> String {
        format!("正在批量同步 {} 个具有相同标签的仓库...", count)
    }
    pub fn reload_repos_failed<E: std::fmt::Display>(e: E) -> String {
        format!("重新加载仓库失败: {}", e)
    }
    pub fn update_tags_failed<E: std::fmt::Display>(e: E) -> String {
        format!("更新标签失败: {}", e)
    }
    pub fn refresh_failed<E: std::fmt::Display>(e: E) -> String {
        format!("刷新失败: {}", e)
    }
    pub fn status_fmt(repo_id: &str, dirty: bool, ahead: usize, behind: usize) -> String {
        format!("[{}] 状态: 未提交={} 超前={} 落后={}", repo_id, dirty, ahead, behind)
    }
    pub fn sync_progress_fmt(repo_id: &str, action: &str, message: &str) -> String {
        format!("[{}] {}: {} - {}", repo_id, PROGRESS, action, message)
    }
}
