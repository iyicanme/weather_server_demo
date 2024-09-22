use std::borrow::Cow;
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;

use rand::seq::IndexedRandom;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use weather_server_lib::config::Config;
use weather_server_lib::http_client::{Coordinate, HttpClient, WeatherApiResponse};

#[tokio::test]
async fn geolocation_api_succeeds_for_non_loopback_ip() {
    let config = Config::read().expect("could not read config");

    let mock_server = MockServer::start().await;

    let latitude = rand::random();
    let longitude = rand::random();

    let responder = GeolocationResponder {
        latitude,
        longitude,
    };

    Mock::given(method("GET"))
        .and(path_regex("/[a-fA-F0-9\\.:]*/latlong"))
        .respond_with(responder)
        .expect(1)
        .mount(&mock_server)
        .await;

    let host = format!(
        "http://{}:{}",
        mock_server.address().ip(),
        mock_server.address().port()
    );
    let client = HttpClient::new_with_hosts(&config.weather_api_key, &host, &host);

    let response = client
        .get_coordinates_for_ip("176.12.12.12")
        .await
        .expect("request to API failed");

    assert!(response.latitude - latitude < 0.000_000_001);
    assert!(response.longitude - longitude < 0.000_000_001);
}

struct GeolocationResponder {
    latitude: f64,
    longitude: f64,
}

impl Respond for GeolocationResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let Some(path_params) = request.url.path().split('/').nth(1) else {
            return ResponseTemplate::new(400);
        };

        let Ok(ip) = IpAddr::from_str(path_params) else {
            return ResponseTemplate::new(400);
        };

        let loopback_v4 =
            IpAddr::from_str("127.0.0.1").expect("should be valid IPv4 loopback address");
        let loopback_v6 = IpAddr::from_str("::1").expect("should be valid IPv6 loopback address");
        if ip.eq(&loopback_v4) || ip.eq(&loopback_v6) {
            return ResponseTemplate::new(200).set_body_string("Undefined,Undefined");
        }

        let response = format!("{},{}", self.latitude, self.longitude);

        ResponseTemplate::new(200).set_body_string(response)
    }
}

#[tokio::test]
async fn weather_api_succeeds() {
    let config = Config::read().expect("could not read config");

    let mock_server = MockServer::start().await;

    let temperature = rand::random();
    let feels_like = rand::random();
    let condition = (*["Sunny", "Cloudy", "Rainy", "Snowy"]
        .choose(&mut rand::thread_rng())
        .unwrap())
    .to_owned();

    let responder = WeatherResponder {
        temperature,
        feels_like,
        condition: condition.clone(),
    };

    Mock::given(method("GET"))
        .and(path("/v1/current.json"))
        .respond_with(responder)
        .expect(1)
        .mount(&mock_server)
        .await;

    let host = format!(
        "http://{}:{}",
        mock_server.address().ip(),
        mock_server.address().port()
    );
    let client = HttpClient::new_with_hosts(&config.weather_api_key, &host, &host);

    let response = client
        .get_weather_for_coordinates(45.0, 45.0)
        .await
        .expect("request to server failed");

    assert!(response.temperature - temperature < 0.000_000_001);
    assert!(response.feels_like - feels_like < 0.000_000_001);
    assert_eq!(response.condition, condition);
}

struct WeatherResponder {
    temperature: f64,
    feels_like: f64,
    condition: String,
}

impl Respond for WeatherResponder {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let queries = request
            .url
            .query_pairs()
            .collect::<HashMap<Cow<str>, Cow<str>>>();

        if !queries.contains_key("key") {
            return ResponseTemplate::new(400);
        }

        let Some(coordinates) = queries.get("q") else {
            return ResponseTemplate::new(400);
        };

        if Coordinate::from_str(coordinates).is_err() {
            return ResponseTemplate::new(400);
        }

        let response = WeatherApiResponse {
            temperature: self.temperature,
            feels_like: self.feels_like,
            condition: self.condition.clone(),
            last_updated: "2024-01-01 00:00".to_owned(),
        };

        ResponseTemplate::new(200).set_body_json(response)
    }
}
