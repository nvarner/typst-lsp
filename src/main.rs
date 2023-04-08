use server::TypstServer;
use tower_lsp::{LspService, Server};

mod command;
mod config;
mod ext;
mod lsp_typst_boundary;
mod server;
mod workspace;

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(TypstServer::with_client);

    Server::new(stdin, stdout, socket).serve(service).await;
}
