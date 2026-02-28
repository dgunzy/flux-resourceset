#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ApiMode {
    ReadOnly,
    Crud,
}

impl ApiMode {
    pub fn from_env(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "readonly" | "read-only" | "read_only" => Ok(Self::ReadOnly),
            "crud" => Ok(Self::Crud),
            other => Err(format!(
                "invalid API_MODE '{other}', expected 'read-only' or 'crud'"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StoreBackend {
    InMemory,
    Sqlite,
}

impl StoreBackend {
    pub fn from_env(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "memory" | "in-memory" | "in_memory" => Ok(Self::InMemory),
            "sqlite" => Ok(Self::Sqlite),
            other => Err(format!(
                "invalid STORE_BACKEND '{other}', expected 'memory' or 'sqlite'"
            )),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub mode: ApiMode,
    pub store_backend: StoreBackend,
    pub database_url: String,
    pub auth_token: String,
    pub crud_auth_token: Option<String>,
    pub seed_file: String,
    pub openapi_file: String,
    pub listen_addr: String,
}

impl Config {
    pub fn from_env() -> Self {
        let mode = std::env::var("API_MODE")
            .map(|v| ApiMode::from_env(&v).expect("invalid API_MODE value"))
            .unwrap_or(ApiMode::ReadOnly);
        let store_backend = std::env::var("STORE_BACKEND")
            .map(|v| StoreBackend::from_env(&v).expect("invalid STORE_BACKEND value"))
            .unwrap_or(StoreBackend::Sqlite);

        Self {
            mode,
            store_backend,
            database_url: std::env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://data/flux-resourceset.db?mode=rwc".into()),
            auth_token: std::env::var("AUTH_TOKEN").expect("AUTH_TOKEN must be set"),
            crud_auth_token: std::env::var("CRUD_AUTH_TOKEN").ok(),
            seed_file: std::env::var("SEED_FILE").unwrap_or_else(|_| "data/seed.json".into()),
            openapi_file: std::env::var("OPENAPI_FILE")
                .unwrap_or_else(|_| "openapi/openapi.yaml".into()),
            listen_addr: std::env::var("LISTEN_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
        }
    }
}
