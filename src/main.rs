use bpaf::{construct, OptionParser, Parser};
use logging::{tracing_init, tracing_shutdown};
use server::log::LspLayer;
use server::TypstServer;
use tower_lsp::{LspService, Server};
use tracing_subscriber::{reload, Registry};

mod command;
mod config;
mod ext;
mod logging;
mod lsp_typst_boundary;
mod server;
mod workspace;

// TODO: is there a good way to get this automatically at build time?
pub const TYPST_VERSION: &str = "0.6.0";

#[tokio::main]
async fn main() {
    let lsp_tracing_layer_handle = tracing_init();
    run(lsp_tracing_layer_handle).await;
    tracing_shutdown();
}

#[tracing::instrument(skip_all)]
async fn run(lsp_tracing_layer_handle: reload::Handle<Option<LspLayer>, Registry>) {
    let _args = arg_parser().run();

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(move |client| TypstServer::new(client, lsp_tracing_layer_handle));

    Server::new(stdin, stdout, socket).serve(service).await;
}

#[derive(Debug, Clone)]
struct Args {}

fn arg_parser() -> OptionParser<Args> {
    construct!(Args {}).to_options().version(
        format!(
            "{} (Typst version {TYPST_VERSION})",
            env!("CARGO_PKG_VERSION")
        )
        .as_str(),
    )
}
