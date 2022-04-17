pub struct Config {
    pub(crate) metrics_port: u32,
    pub(crate) db_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            metrics_port: int_from_env("METRICS_PORT").unwrap_or(4000),
            db_url: std::env::var("DB_URL")
                .unwrap_or_else(|_| "pg://poe:poe@trade-store/poe".into()),
        })
    }
}

fn int_from_env(key: &str) -> Result<u32, Box<dyn std::error::Error>> {
    std::env::var(key)?.parse::<u32>().map_err(|e| e.into())
}