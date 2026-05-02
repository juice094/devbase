use crate::tui::{App, MainView};

impl App {
    pub(crate) fn next(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = (self.selected + 1) % self.repos.len();
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected = (self.vault_selected + 1) % self.vaults.len();
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }

    pub(crate) fn previous(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = (self.selected + self.repos.len() - 1) % self.repos.len();
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected =
                        (self.vault_selected + self.vaults.len() - 1) % self.vaults.len();
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }

    pub(crate) fn jump_to_top(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = 0;
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected = 0;
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }

    pub(crate) fn jump_to_bottom(&mut self) {
        match self.main_view {
            MainView::RepoList => {
                if !self.repos.is_empty() {
                    self.selected = self.repos.len() - 1;
                    self.list_state.select(Some(self.selected));
                    self.spawn_repo_status_for_current();
                }
            }
            MainView::VaultList => {
                if !self.vaults.is_empty() {
                    self.vault_selected = self.vaults.len() - 1;
                    self.vault_list_state.select(Some(self.vault_selected));
                }
            }
        }
    }
}
