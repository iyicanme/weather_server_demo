use poem::listener::TcpListener;
use poem::{Route, Server};
use poem_openapi::OpenApiService;
use sqlx::{Sqlite, SqlitePool};
use sqlx::migrate::MigrateDatabase;
use tracing::log::{Level, log};

use crate::Api;
use crate::config::Config;
use crate::http_client::HttpClient;

pub async fn setup() -> Result<PendingServer, anyhow::Error> {
    let config = Config::read().unwrap();

    let database = database(&config.database_name).await?;

    let http_client = HttpClient::new(&config.weather_api_key);
    let api = Api::new(http_client, database);

    let api_service =
        OpenApiService::new(api, "Weather Server Demo", "1.0").server("http://localhost:3000/api");
    let ui = api_service.swagger_ui();

    let routes = Route::new().nest("/api", api_service).nest("/", ui);

    let address = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(address);
    
    Ok(PendingServer {listener, routes})
}

pub struct PendingServer {
    listener: TcpListener<String>,
    routes: Route,
}

impl PendingServer {
    pub async fn serve(self) -> Result<(), std::io::Error> {
        Server::new(self.listener).run(self.routes).await
    }
}

async fn database(database_name: &str) -> Result<SqlitePool, sqlx::Error> {
    let database_url = format!("sqlite://{}", database_name);
    if Sqlite::database_exists(&database_url).await.unwrap_or(false) {
        log!(Level::Info, "Database found");
    } else {
        log!(Level::Warn, "Database is missing! Creating database");
        Sqlite::create_database(&database_url)
            .await
            .expect("Database creation failed. Can not proceed without a database");
        log!(Level::Info, "Database created");
    }

    SqlitePool::connect(&database_url).await
}