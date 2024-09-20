use poem::{Route, Server};
use poem::listener::TcpListener;
use poem_openapi::OpenApiService;

use weather_server_lib::Api;
use weather_server_lib::config::Config;
use weather_server_lib::http_client::HttpClient;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    if std::env::var_os("RUST_LOG").is_none() {
        unsafe {
            std::env::set_var("RUST_LOG", "poem=debug");
        }
    }

    tracing_subscriber::fmt::init();

    let config = Config::read().unwrap();
    let http_client = HttpClient::new(&config.weather_api_key);
    let api = Api::new(http_client);

    let api_service =
        OpenApiService::new(api, "Weather Server Demo", "1.0").server("http://localhost:3000/api");
    let ui = api_service.swagger_ui();

    let routes = Route::new().nest("/api", api_service).nest("/", ui);

    Server::new(TcpListener::bind(format!("0.0.0.0:{}", config.port)))
        .run(routes)
        .await
}
