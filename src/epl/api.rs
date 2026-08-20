use std::sync::OnceLock;

use crate::{api::FootballDataApi, types::Data};

pub fn football_data(data: &Data) -> FootballDataApi {
    static TOKEN: OnceLock<String> = OnceLock::new();
    let token = TOKEN.get_or_init(|| {
        std::env::var("FOOTBALL_DATA_API_TOKEN").expect(
            "FOOTBALL_DATA_API_TOKEN must be set (required while soccer leagues are compiled in)",
        )
    });
    FootballDataApi::new(data.http.clone(), token)
}
