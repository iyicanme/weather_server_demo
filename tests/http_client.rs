use rand::seq::SliceRandom;
use std::borrow::Cow;
use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use wiremock::matchers::{method, path, path_regex};
use wiremock::{Mock, MockServer, Request, Respond, ResponseTemplate};

use weather_server_lib::http_client::{
    Condition, Coordinate, Current, HttpClient, Location, WeatherApiResponse,
};
use weather_server_lib::helpers::is_loopback_address;

#[tokio::test]
async fn geolocation_api_succeeds_for_non_loopback_ip() {
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
    let client = HttpClient::new_with_hosts(&host, &host).expect("could not create HTTP client");

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

        if is_loopback_address(&ip) {
            return ResponseTemplate::new(200).set_body_string("Undefined,Undefined");
        }

        let response = format!("{},{}", self.latitude, self.longitude);

        ResponseTemplate::new(200).set_body_string(response)
    }
}

#[tokio::test]
async fn weather_api_succeeds() {
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
    let client = HttpClient::new_with_hosts(&host, &host).expect("could not create HTTP client");

    let response = client
        .get_weather_for_coordinates(45.0, 45.0)
        .await
        .expect("request to server failed");

    assert!(response.current.temp_c - temperature < 0.000_000_001);
    assert!(response.current.feelslike_c - feels_like < 0.000_000_001);
    assert_eq!(response.current.condition.text, condition);
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
            location: Location,
            current: Current {
                last_updated: String::new(),
                temp_c: self.temperature,
                condition: Condition {
                    text: self.condition.clone(),
                },
                feelslike_c: self.feels_like,
            },
        };

        ResponseTemplate::new(200).set_body_json(response)
    }
}
