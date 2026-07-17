use std::sync::OnceLock;

use crate::{api::FootballDataApi, types::Data};

/// football-data.org client for the World Cup league module.
///
/// Token is read from `FOOTBALL_DATA_API_TOKEN` (fail-fast at process start in `main`).
pub fn football_data(data: &Data) -> FootballDataApi {
    static TOKEN: OnceLock<String> = OnceLock::new();
    let token = TOKEN.get_or_init(|| {
        std::env::var("FOOTBALL_DATA_API_TOKEN").expect(
            "FOOTBALL_DATA_API_TOKEN must be set (required while the wc league is compiled in)",
        )
    });
    FootballDataApi::new(data.http.clone(), token)
}
