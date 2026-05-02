use crate::tui::{App, VaultItem};

impl App {
    pub(crate) fn spawn_vault_watcher(&mut self) {
        let vault_path = match crate::registry::WorkspaceRegistry::workspace_dir() {
            Ok(ws) => ws.join("vault"),
            Err(_) => return,
        };
        if !vault_path.exists() {
            return;
        }
        let tx = self.async_tx.clone();
        std::thread::spawn(move || {
            let watcher = match crate::watch::FsWatcher::new(&vault_path) {
                Ok(w) => w,
                Err(_) => return,
            };
            loop {
                if watcher.poll_event(std::time::Duration::from_secs(2)).is_some() {
                    // Debounce: wait 500ms then drain remaining events
                    std::thread::sleep(std::time::Duration::from_millis(500));
                    let _ = watcher.poll_event(std::time::Duration::from_millis(100));
                    let _ = tx.send(crate::asyncgit::AsyncNotification::VaultChanged);
                }
            }
        });
    }

    pub(crate) fn current_vault(&self) -> Option<&VaultItem> {
        self.vaults.get(self.vault_selected)
    }

    pub(crate) fn load_vaults(&mut self) -> anyhow::Result<()> {
        let conn = self.ctx.conn()?;
        let notes = crate::registry::vault::list_vault_notes(&conn)?;
        self.vaults.clear();
        for note in notes {
            self.vaults.push(VaultItem {
                id: note.id,
                path: note.path,
                title: note.title,
                tags: note.tags,
                outgoing_links: note.outgoing_links,
            });
        }
        self.vault_selected = 0;
        self.vault_list_state.select(Some(0));
        self.log_info(self.ctx.i18n.log.loaded_vaults(self.vaults.len()));
        Ok(())
    }
}
