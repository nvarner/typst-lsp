use anyhow::Result;
use lapce_plugin::psp_types::lsp_types::request::Initialize;
use lapce_plugin::psp_types::lsp_types::{DocumentFilter, InitializeParams, MessageType, Url};
use lapce_plugin::psp_types::Request;
use lapce_plugin::{register_plugin, LapcePlugin, VoltEnvironment, PLUGIN_RPC};
use serde::Deserialize;
use serde_json::Value;
use server_init::get_server_path;

mod server_init;

#[derive(Default)]
pub struct State;

register_plugin!(State);

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TypstLspOptions {
    pub server_path: Option<String>,
}

fn initialize(params: InitializeParams) -> Result<()> {
    PLUGIN_RPC.window_show_message(MessageType::INFO, format!("init params: {params:#?}"));

    let document_selector = vec![DocumentFilter {
        language: None, // TODO: if Typst is implemented in Lapce, require it here, not in pattern
        scheme: None,
        pattern: Some("**/*.typ".to_owned()),
    }];

    let typst_lsp_options = params
        .initialization_options
        .as_ref()
        .and_then(|options| options.get("typst-lsp").cloned())
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();

    let server_path = get_server_path(&typst_lsp_options)?;

    let base_uri = VoltEnvironment::uri()?;
    let server_uri = Url::parse(&base_uri)?.join(&server_path)?;

    PLUGIN_RPC.start_lsp(
        server_uri,
        vec![],
        document_selector,
        params.initialization_options,
    );

    Ok(())
}

impl State {
    fn dispatch_request(&mut self, _id: u64, method: String, params: Value) -> Result<()> {
        match method.as_str() {
            Initialize::METHOD => {
                let params = serde_json::from_value(params)
                    .expect("initialize method should have `InitializeParams` params");
                initialize(params)
            }
            _ => Ok(()),
        }
    }
}

impl LapcePlugin for State {
    fn handle_request(&mut self, id: u64, method: String, params: Value) {
        if let Err(err) = self.dispatch_request(id, method, params) {
            PLUGIN_RPC.window_show_message(
                MessageType::ERROR,
                format!("Typst LSP plugin error: {err:?}"),
            );
        }
    }
}
