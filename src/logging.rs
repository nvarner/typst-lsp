use tracing_subscriber::prelude::__tracing_subscriber_SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{reload, Registry};

use crate::server::log::LspLayer;

pub fn tracing_init() -> reload::Handle<Option<LspLayer>, Registry> {
    let (lsp_layer, lsp_layer_handle) = reload::Layer::new(None);
    let jaeger_layer = jaeger::init();

    tracing_subscriber::registry()
        .with(lsp_layer)
        .with(jaeger_layer)
        .init();

    lsp_layer_handle
}

pub fn tracing_shutdown() {
    #[cfg(feature = "jaeger")]
    opentelemetry::global::shutdown_tracer_provider();
}

#[cfg(feature = "jaeger")]
mod jaeger {
    use tracing::Subscriber;
    use tracing_subscriber::registry::LookupSpan;
    use tracing_subscriber::Layer;

    pub fn init<S: Subscriber + for<'a> LookupSpan<'a>>() -> Option<impl Layer<S>> {
        opentelemetry::global::set_text_map_propagator(opentelemetry_jaeger::Propagator::new());

        opentelemetry_jaeger::new_collector_pipeline()
            .with_endpoint("http://localhost:14268/api/traces")
            .with_service_name("typst-lsp")
            .with_isahc()
            .install_batch(opentelemetry::runtime::Tokio)
            .ok()
            .map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer))
    }
}

#[cfg(not(feature = "jaeger"))]
mod jaeger {
    use tracing::Subscriber;
    use tracing_subscriber::Layer;

    pub fn init<S: Subscriber>() -> Option<impl Layer<S>> {
        Some(IdLayer)
    }

    pub struct IdLayer;

    impl<S: Subscriber> Layer<S> for IdLayer {}
}
