use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Mutex;

use crate::api::{EspnApi, FootballDataApi};

#[derive(Clone)]
pub struct Data {
    pub db: Arc<Mutex<Connection>>,
    pub http: reqwest::Client,
    pub api_token: String,
}

impl Data {
    pub fn soccar_api(&self) -> FootballDataApi {
        FootballDataApi::new(self.http.clone(), &self.api_token)
    }

    pub fn espn_api(&self) -> EspnApi {
        EspnApi::new(self.http.clone())
    }
}

pub type Error = Box<dyn std::error::Error + Send + Sync>;
pub type Context<'a> = poise::Context<'a, Data, Error>;
