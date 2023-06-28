use server::log::LspLayer;
use server::TypstServer;
use tower_lsp::{LspService, Server};
use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{reload, Registry};

mod command;
mod config;
mod ext;
mod lsp_typst_boundary;
mod server;
mod workspace;

#[tokio::main]
async fn main() {
    let lsp_tracing_layer_handle = tracing_init();
    run(lsp_tracing_layer_handle).await;
    opentelemetry::global::shutdown_tracer_provider();
}

#[tracing::instrument(skip_all)]
async fn run(lsp_tracing_layer_handle: reload::Handle<Option<LspLayer>, Registry>) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) =
        LspService::new(move |client| TypstServer::new(client, lsp_tracing_layer_handle));

    Server::new(stdin, stdout, socket).serve(service).await;
}

fn tracing_init() -> reload::Handle<Option<LspLayer>, Registry> {
    opentelemetry::global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

    let tracer = opentelemetry_jaeger::new_collector_pipeline()
        .with_endpoint("http://localhost:14268/api/traces")
        .with_service_name("typst-lsp")
        .with_isahc()
        .install_batch(opentelemetry::runtime::Tokio)
        .ok();

    let (lsp_layer, lsp_layer_handle) = reload::Layer::new(None);

    tracing_subscriber::registry()
        .with(lsp_layer)
        .with(tracer.map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer)))
        .init();

    lsp_layer_handle
}
