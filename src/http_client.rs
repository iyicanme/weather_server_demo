use std::collections::HashMap;
use std::env::VarError;
use std::str::FromStr;

use reqwest::StatusCode;

/// Wrapper for foreign API accesses.
pub struct HttpClient {
    client: reqwest::Client,
    weather_api_key: String,
    geolocation_api_host: String,
    weather_api_host: String,
}

impl HttpClient {
    /// Default geolocation API hostname.
    const GEOLOCATION_API_HOST: &'static str = "https://ipapi.co";

    /// Default weather API hostname.
    const WEATHER_API_HOST: &'static str = "https://api.weatherapi.com";

    /// Creates a `HTTPClient` instance with default hostnames.
    /// 
    /// # Errors
    /// Returns an error if environment variable `WEATHER_API_KEY` is not set.
    pub fn new() -> Result<Self, VarError> {
        Self::new_with_hosts(Self::GEOLOCATION_API_HOST, Self::WEATHER_API_HOST)
    }

    /// Creates a `HTTPClient` instance with given foreign API hostnames.
    /// 
    /// Used in testing to enable the ability to direct the calls to a local endpoint.
    /// 
    /// # Errors
    /// Returns an error if environment variable `WEATHER_API_KEY` is not set.
    pub fn new_with_hosts(
        geolocation_api_host: &str,
        weather_api_host: &str,
    ) -> Result<Self, VarError> {
        let weather_api_key = std::env::var("WEATHER_API_KEY")?;
        let client = Self {
            client: reqwest::Client::default(),
            weather_api_key,
            geolocation_api_host: geolocation_api_host.to_owned(),
            weather_api_host: weather_api_host.to_owned(),
        };

        Ok(client)
    }

    /// Makes a call to the geolocation API, parses the response and returns the coordinates.
    /// 
    /// Expected response format is `LATITUDE,LONGITUDE`.
    /// 
    /// # Errors
    /// Returns an error if:
    /// - Call to endpoint fails
    /// - Response status code is not `200 Success`
    /// - The response does not include a body
    /// - Response has unexpected format
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

    /// Makes a call to weather API and returns the response.
    /// 
    /// # Errors
    /// Will fail if:
    /// - Call to endpoint fails
    /// - The body is not in expected format
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

/// The response HTTP client returns from geolocation API call.
#[derive(serde::Deserialize)]
pub struct GeolocationApiResponse {
    pub latitude: f64,
    pub longitude: f64,
}

/// The response HTTP client returns from weather API call.
/// 
/// The API is configured to return only the desired information
/// but can be configured to return more.
/// 
/// The API also returns information about the location of the coordinates, but they are discarded.
#[derive(serde::Deserialize, serde::Serialize)]
pub struct WeatherApiResponse {
    #[serde(skip)]
    pub location: Location,
    pub current: Current,
}

/// A placeholder type, used in `WeatherApiResponse`
/// so `location` section of the response can be discarded.
#[derive(Default)]
pub struct Location;

/// The information the API returns about the weather at given location
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Current {
    pub last_updated: String,
    pub temp_c: f64,
    pub condition: Condition,
    pub feelslike_c: f64,
}

/// The information about weather condition
#[derive(serde::Deserialize, serde::Serialize)]
pub struct Condition {
    pub text: String,
}

/// HTTP client errors
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

/// Represents a coordinate, used to parse the geolocation API response
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
