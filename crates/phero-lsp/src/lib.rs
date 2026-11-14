//! phero-lsp — language server for pherolang `.mox` models.

pub mod hover;
pub mod position;
pub mod server;

pub use server::PheroBackend;

use tower_lsp_server::{LspService, Server};

/// Serves the pherolang language server over stdin/stdout, using the
/// `Content-Length` framing required by the LSP. Blocks until the client
/// sends `exit` (or closes stdin).
///
/// # Errors
///
/// Returns an I/O error when the async runtime or the standard streams
/// cannot be set up.
pub fn run_stdio() -> std::io::Result<()> {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let (service, socket) = LspService::new(PheroBackend::new);
        Server::new(tokio::io::stdin(), tokio::io::stdout(), socket)
            .serve(service)
            .await;
    });
    Ok(())
}
