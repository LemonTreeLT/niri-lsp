use niri_lsp::Backend;
use tower_lsp::{LspService, Server};

#[tokio::main]
async fn main() {
    if std::env::args().any(|a| a == "--version" || a == "-V") {
        println!(
            "niri-lsp {} (niri {})",
            env!("CARGO_PKG_VERSION"),
            niri_lsp::NIRI_REF
        );
        return;
    }
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let (service, socket) = LspService::new(Backend::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
