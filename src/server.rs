use poem::listener::TcpListener;
use poem::{Route, Server};
use poem_openapi::OpenApiService;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};

use crate::config::Config;
use crate::http_client::HttpClient;
use crate::Api;

pub async fn setup(config: &Config) -> Result<PendingServer, anyhow::Error> {
    let database = database(&config.database_name).await?;

    let http_client = HttpClient::new(&config.weather_api_key);
    let api = Api::new(http_client, database.clone());

    let api_service =
        OpenApiService::new(api, "Weather Server Demo", "1.0").server("http://localhost:3000/api");
    let ui = api_service.swagger_ui();

    let routes = Route::new().nest("/api", api_service).nest("/", ui);

    let address = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(address);

    Ok(PendingServer { listener, routes, database })
}

pub struct PendingServer {
    listener: TcpListener<String>,
    routes: Route,
    database: SqlitePool,
}

impl PendingServer {
    pub async fn serve(self) -> Result<(), std::io::Error> {
        Server::new(self.listener).run(self.routes).await
    }

    #[must_use]
    pub fn database(&self) -> SqlitePool {
        self.database.clone()
    }
}

async fn database(database_name: &str) -> Result<SqlitePool, sqlx::Error> {
    let database_url = format!("sqlite://{database_name}.db");

    let database_exists = Sqlite::database_exists(&database_url).await.unwrap_or(false);
    if !database_exists {
        Sqlite::create_database(&database_url).await?;
    }

    let database = SqlitePool::connect(&database_url).await?;
    sqlx::migrate!("./migrations").run(&database).await?;

    Ok(database)
}