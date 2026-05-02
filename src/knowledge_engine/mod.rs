pub mod fallback;
pub mod index;
pub mod llm;
pub mod module;
pub mod readme;

pub use fallback::*;
pub use index::*;
pub use llm::*;
pub use module::*;
pub use readme::*;

#[derive(Debug, Clone)]
pub struct ModuleInfo {
    pub name: String,
    pub kind: String, // "lib", "bin", "test", "example", "unknown"
}

/// Run an async future from a synchronous context safely.
///
/// If already inside a tokio runtime (e.g. `spawn_blocking`), spawns the
/// future onto that runtime and blocks the current thread on a std channel.
/// If outside any runtime, creates a temporary runtime.
pub(crate) fn block_on_async<T>(
    future: impl std::future::Future<Output = T> + Send + 'static,
) -> Option<T>
where
    T: Send + 'static,
{
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let (tx, rx) = std::sync::mpsc::channel();
            handle.spawn(async move {
                let _ = tx.send(future.await);
            });
            rx.recv().ok()
        }
        Err(_) => {
            let rt = tokio::runtime::Runtime::new().ok()?;
            Some(rt.block_on(future))
        }
    }
}
