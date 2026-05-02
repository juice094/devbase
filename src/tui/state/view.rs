use crate::tui::App;

impl App {
    pub(crate) fn toggle_main_view(&mut self) {
        self.main_view = self.main_view.toggle();
    }

    pub(crate) fn toggle_help(&mut self) {
        self.help_popup_mode = match self.help_popup_mode {
            crate::tui::HelpPopupMode::Hidden => crate::tui::HelpPopupMode::Visible,
            crate::tui::HelpPopupMode::Visible => crate::tui::HelpPopupMode::Hidden,
        };
    }

    pub(crate) fn update_tags(&mut self, new_tags: &str) {
        let repo_id = match self.current_repo() {
            Some(r) => r.id.clone(),
            None => {
                self.log_warn(self.ctx.i18n.log.no_repo_selected.to_string());
                return;
            }
        };

        match (|| -> anyhow::Result<()> {
            let mut conn = self.ctx.conn_mut()?;
            let tx = conn.transaction()?;
            tx.execute("DELETE FROM repo_tags WHERE repo_id = ?1", [&repo_id])?;
            for tag in new_tags.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()) {
                tx.execute(
                    "INSERT OR REPLACE INTO repo_tags (repo_id, tag) VALUES (?1, ?2)",
                    rusqlite::params![&repo_id, tag],
                )?;
            }
            tx.commit()?;
            Ok(())
        })() {
            Ok(()) => {
                self.log_info(self.ctx.i18n.log.updated_tags(&repo_id, new_tags));
                if let Err(e) = self.load_repos() {
                    self.log_error(self.ctx.i18n.log.reload_repos_failed(e));
                }
            }
            Err(e) => self.log_error(self.ctx.i18n.log.update_tags_failed(e)),
        }
    }
}
