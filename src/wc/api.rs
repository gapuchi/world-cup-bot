use crate::{api::FootballDataApi, types::Data};

/// football-data.org client for the World Cup league module.
pub fn football_data(data: &Data) -> FootballDataApi {
    FootballDataApi::from_env(data.http.clone())
}
