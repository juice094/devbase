use std::sync::OnceLock;

#[allow(dead_code)]
pub struct I18n {
    pub tui: TuiStrings,
    pub cli: CliStrings,
    pub sync: SyncStrings,
    pub log: LogStrings,
}

#[allow(dead_code)]
pub struct TuiStrings {
    pub title_repos: &'static str,
    pub title_details: &'static str,
    pub title_logs: &'static str,
    pub title_sync_progress: &'static str,
    pub label_id: &'static str,
    pub label_path: &'static str,
    pub label_branch: &'static str,
    pub label_tags: &'static str,
    pub label_language: &'static str,
    pub label_upstream: &'static str,
    pub label_status: &'static str,
    pub status_loading: &'static str,
    pub status_none: &'static str,
    pub status_no_upstream: &'static str,
    pub status_ok: &'static str,
    pub status_dirty: &'static str,
    pub status_unknown: &'static str,
    pub hint_tag_input: &'static str,
    pub hint_popup_close: &'static str,
    pub help_quit: &'static str,
    pub help_refresh: &'static str,
    pub help_preview: &'static str,
    pub help_batch: &'static str,
    pub help_tag: &'static str,
    pub help_help: &'static str,
    pub help_navigate: &'static str,
}

#[allow(dead_code)]
pub struct CliStrings {
    pub scanning: &'static str,
    pub health_check: &'static str,
    pub syncing: &'static str,
    pub querying: &'static str,
    pub indexing: &'static str,
    pub launching_tui: &'static str,
    pub generating_digest: &'static str,
}

#[allow(dead_code)]
pub struct SyncStrings {
    pub strategy_prefix: &'static str,
    pub filter_prefix: &'static str,
    pub skip_no_upstream: &'static str,
    pub checking: &'static str,
    pub error_prefix: &'static str,
    pub fetched_only: &'static str,
    pub blocked_dirty: &'static str,
    pub merged_ff: &'static str,
    pub merged_commit: &'static str,
    pub conflict: &'static str,
    pub dry_run_complete: &'static str,
    pub sync_complete: &'static str,
    pub no_repos_processed: &'static str,
    pub header_repo: &'static str,
    pub header_action: &'static str,
    pub header_ahead: &'static str,
    pub header_behind: &'static str,
    pub header_message: &'static str,
    pub summary_total: &'static str,
    pub summary_success: &'static str,
    pub summary_errors: &'static str,
    pub summary_skipped: &'static str,
    pub would_fetch: &'static str,
    pub fetch_success: &'static str,
    pub fetch_failed: &'static str,
    pub network_timeout: &'static str,
    pub no_origin: &'static str,
    pub status_running: &'static str,
    pub up_to_date: &'static str,
    pub skipped_by_user: &'static str,
    pub local_branch_missing: &'static str,
    pub remote_branch_missing: &'static str,
    pub neither_branch_exists: &'static str,
    pub already_up_to_date: &'static str,
    pub unhandled_merge_state: &'static str,
}

#[allow(dead_code)]
pub struct LogStrings {
    pub tui_started: &'static str,
    pub no_repo_selected: &'static str,
    pub refreshing: &'static str,
    pub tag_input_cancelled: &'static str,
    pub empty_tag_ignored: &'static str,
    pub no_tags_to_sync: &'static str,
    pub no_repos_match_tags: &'static str,
    pub sync_finished: &'static str,
    pub progress: &'static str,
    pub no_repos_registered: &'static str,
    pub status_queued: &'static str,
    pub health_summary: &'static str,
    pub health_environment: &'static str,
    pub health_repos: &'static str,
    pub not_installed: &'static str,
    pub digest_title: &'static str,
    pub digest_generated_at: &'static str,
    pub digest_new_repos: &'static str,
    pub digest_unhealthy_repos: &'static str,
    pub digest_no_summary: &'static str,
    pub digest_new_discoveries: &'static str,
    pub digest_global: &'static str,
    pub digest_overall: &'static str,
    pub digest_failed: &'static str,
    pub digest_panic: &'static str,
}

#[allow(dead_code)]
impl LogStrings {
    pub fn loaded_repos(&self, count: usize) -> String {
        format!("已加载 {} 个仓库。", count)
    }
    pub fn fetching_preview(&self, repo_id: &str) -> String {
        format!("正在为 {} 获取预览...", repo_id)
    }
    pub fn preview_done(&self, repo_id: &str, ahead: usize, behind: usize) -> String {
        format!("{} 的预览：ahead={} behind={}", repo_id, ahead, behind)
    }
    pub fn updated_tags(&self, repo_id: &str, tags: &str) -> String {
        format!("已更新 [{}] 的标签为：{}", repo_id, tags)
    }
    pub fn batch_syncing(&self, count: usize) -> String {
        format!("正在批量同步 {} 个具有相同标签的仓库...", count)
    }
    pub fn reload_repos_failed<E: std::fmt::Display>(&self, e: E) -> String {
        format!("重新加载仓库失败: {}", e)
    }
    pub fn update_tags_failed<E: std::fmt::Display>(&self, e: E) -> String {
        format!("更新标签失败: {}", e)
    }
    pub fn refresh_failed<E: std::fmt::Display>(&self, e: E) -> String {
        format!("刷新失败: {}", e)
    }
    pub fn status_fmt(&self, repo_id: &str, dirty: bool, ahead: usize, behind: usize) -> String {
        format!("[{}] 状态: 未提交={} 超前={} 落后={}", repo_id, dirty, ahead, behind)
    }
    pub fn sync_progress_fmt(&self, repo_id: &str, action: &str, message: &str) -> String {
        format!("[{}] {}: {} - {}", repo_id, self.progress, action, message)
    }
}

pub mod en;
pub mod zh_cn;

static CURRENT: OnceLock<I18n> = OnceLock::new();

pub fn init(lang: &str) {
    let lang_lower = lang.to_lowercase();
    let i18n = if lang_lower.starts_with("en") {
        crate::i18n::en::build()
    } else {
        crate::i18n::zh_cn::build()
    };
    let _ = CURRENT.set(i18n);
}

pub fn current() -> &'static I18n {
    CURRENT.get().expect("i18n not initialized")
}

pub fn format_template(template: &str, args: &[&str]) -> String {
    let mut result = template.to_string();
    for arg in args {
        if let Some(pos) = result.find("{}") {
            result.replace_range(pos..pos+2, arg);
        }
    }
    result
}

pub fn detect_system_language() -> String {
    #[cfg(target_os = "windows")]
    {
        if let Ok(output) = std::process::Command::new("reg")
            .args(&["query", "HKCU\\Control Panel\\Desktop", "/v", "PreferredUILanguages"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("PreferredUILanguages") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(last) = parts.last() {
                        return last.to_lowercase();
                    }
                }
            }
        }
        if let Ok(output) = std::process::Command::new("reg")
            .args(&["query", "HKCU\\Control Panel\\International", "/v", "LocaleName"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                if line.contains("LocaleName") {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if let Some(last) = parts.last() {
                        return last.to_lowercase();
                    }
                }
            }
        }
    }
    std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_else(|_| "zh-CN".to_string())
        .split('.')
        .next()
        .unwrap_or("zh-CN")
        .replace('_', "-")
        .to_lowercase()
}
