use tower_lsp::lsp_types::SemanticToken;

#[derive(Debug)]
struct CachedTokens {
    tokens: Vec<SemanticToken>,
    id: u64,
}

#[derive(Default, Debug)]
pub struct Cache {
    last_sent: Option<CachedTokens>,
    next_id: u64,
}

impl Cache {
    pub fn try_take_result(&mut self, id: &str) -> Option<Vec<SemanticToken>> {
        let id = id.parse::<u64>().ok()?;
        match self.last_sent.take() {
            Some(cached) if cached.id == id => Some(cached.tokens),
            Some(cached) => {
                // replace after taking
                self.last_sent = Some(cached);
                None
            }
            None => None,
        }
    }

    pub fn cache_result(&mut self, tokens: Vec<SemanticToken>) -> String {
        let id = self.get_next_id();
        let cached = CachedTokens { tokens, id };
        self.last_sent = Some(cached);
        id.to_string()
    }

    fn get_next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}
