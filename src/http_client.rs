use std::collections::HashMap;

use crate::http_client::Error::ApiInternalError;

pub struct HttpClient {
    client: reqwest::Client,
    weather_api_key: String,
}

impl HttpClient {
    pub fn new(weather_api_key: &str) -> Self {
        Self {
            client: reqwest::Client::default(),
            weather_api_key: weather_api_key.to_owned(),
        }
    }

    pub async fn get_coordinates_for_ip(&self, ip: &str) -> Result<GeolocationApiResponse, Error> {
        let url = format!("https://ipapi.co/{ip}/latlong/");

        let response = self.client.get(url)
            .send()
            .await
            .map_err(|_| Error::RequestFailed)?
            .text()
            .await
            .map_err(|_| Error::ParsingFailed)?;

        let Some((latitude, longitude)) = response.split_once(',') else {
            return Err(Error::ParsingFailed)
        };

        let latitude = latitude.parse().map_err(|_| Error::ParsingFailed)?;
        let longitude = longitude.parse().map_err(|_| Error::ParsingFailed)?;

        Ok(GeolocationApiResponse { latitude, longitude })
    }

    pub async fn get_weather_for_coordinates(&self, latitude: f64, longitude: f64) -> Result<WeatherApiResponse, Error> {
        let url = "https://api.weatherapi.com/v1/current.json".to_string();

        let mut query_parameters = HashMap::new();
        let location_query = format!("{latitude},{longitude}");
        query_parameters.insert("q", location_query);
        query_parameters.insert("key", self.weather_api_key.clone());

        let response = self.client.get(url)
            .query(&query_parameters)
            .send()
            .await
            .map_err(|_| Error::RequestFailed)?
            .json::<WeatherApiResponse>()
            .await
            .map_err(|_| Error::JsonParsingFailed)?;

        Ok(response)
    }
}

#[derive(serde::Deserialize)]
pub struct GeolocationApiResponse {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(serde::Deserialize)]
pub struct WeatherApiResponse {}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("API request failed")]
    RequestFailed,
    #[error("parsing API response failed")]
    ParsingFailed,
    #[error("parsing API response failed")]
    JsonParsingFailed,
    #[error("API internal error: {0}")]
    ApiInternalError(String),
}