use crate::{api::FootballDataApi, types::Data};

pub fn football_data(data: &Data) -> FootballDataApi {
    FootballDataApi::from_env(data.http.clone())
}
