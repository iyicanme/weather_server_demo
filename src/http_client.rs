use std::collections::HashMap;
use std::str::FromStr;

use reqwest::StatusCode;

pub struct HttpClient {
    client: reqwest::Client,
    weather_api_key: String,
    geolocation_api_host: String,
    weather_api_host: String,
}

impl HttpClient {
    const GEOLOCATION_API_HOST: &'static str = "https://ipapi.co";
    const WEATHER_API_HOST: &'static str = "https://api.weatherapi.com";

    #[must_use]
    pub fn new(weather_api_key: &str) -> Self {
        Self::new_with_hosts(
            weather_api_key,
            Self::GEOLOCATION_API_HOST,
            Self::WEATHER_API_HOST,
        )
    }

    #[must_use]
    pub fn new_with_hosts(
        weather_api_key: &str,
        geolocation_api_host: &str,
        weather_api_host: &str,
    ) -> Self {
        Self {
            client: reqwest::Client::default(),
            weather_api_key: weather_api_key.to_owned(),
            geolocation_api_host: geolocation_api_host.to_owned(),
            weather_api_host: weather_api_host.to_owned(),
        }
    }

    pub async fn get_coordinates_for_ip(&self, ip: &str) -> Result<GeolocationApiResponse, Error> {
        let url = format!("{}/{ip}/latlong/", self.geolocation_api_host);

        let response = self
            .client
            .get(url)
            .send()
            .await
            .map_err(|_| Error::RequestFailed)?;

        let status_code = response.status();
        if status_code != StatusCode::OK {
            return Err(Error::ApiInternalError(format!(
                "API returned {status_code}"
            )));
        }

        let response = response.text().await.map_err(|_| Error::RequestFailed)?;
        let coordinate = Coordinate::from_str(&response)?;

        let response = GeolocationApiResponse {
            latitude: coordinate.latitude,
            longitude: coordinate.longitude,
        };

        Ok(response)
    }

    pub async fn get_weather_for_coordinates(
        &self,
        latitude: f64,
        longitude: f64,
    ) -> Result<WeatherApiResponse, Error> {
        let url = format!("{}/v1/current.json", self.weather_api_host);

        let mut query_parameters = HashMap::new();
        let location_query = format!("{latitude},{longitude}");
        query_parameters.insert("q", location_query);
        query_parameters.insert("key", self.weather_api_key.clone());

        self.client
            .get(url)
            .query(&query_parameters)
            .send()
            .await
            .map_err(|_| Error::RequestFailed)?
            .json::<WeatherApiResponse>()
            .await
            .map_err(|_| Error::JsonParsingFailed)
    }
}

#[derive(serde::Deserialize)]
pub struct GeolocationApiResponse {
    pub latitude: f64,
    pub longitude: f64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct WeatherApiResponse {
    #[serde(skip)]
    pub location: Location,
    pub current: Current,
}

// We need a dummy location struct so we can tell serde to skip de/serialize it
#[derive(Default)]
pub struct Location;

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Current {
    pub last_updated: String,
    pub temp_c: f64,
    pub condition: Condition,
    pub feelslike_c: f64,
}

#[derive(serde::Deserialize, serde::Serialize)]
pub struct Condition {
    pub text: String,
}

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

pub struct Coordinate {
    pub latitude: f64,
    pub longitude: f64,
}

impl FromStr for Coordinate {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let Some((latitude, longitude)) = s.split_once(',') else {
            return Err(Error::ParsingFailed);
        };

        let latitude = latitude.parse().map_err(|_| Error::ParsingFailed)?;
        let longitude = longitude.parse().map_err(|_| Error::ParsingFailed)?;

        let coordinate = Self {
            latitude,
            longitude,
        };

        Ok(coordinate)
    }
}
