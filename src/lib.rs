use crate::api::Api;
use crate::config::Config;
use crate::http_client::HttpClient;
use poem::listener::TcpListener;
use poem::{Route, Server};
use poem_openapi::OpenApiService;
use sqlx::migrate::MigrateDatabase;
use sqlx::{Sqlite, SqlitePool};

/// Request handlers and types they receive and return
pub mod api;
/// Creation and checking of JWT tokens
pub mod authorization;
/// Configuration parameters and reader
pub mod config;
/// Helper functions
pub mod helpers;
/// HTTP client wrapping the geolocation and weather APIs
pub mod http_client;
/// Hashing and checking of hashed passwords
pub mod password;
/// Wrappers for database queries
pub mod queries;


/// Initialization operations to get the server ready to run.
///
/// It returns a `PendingServer` instance, which can be used to start the server.
///
/// Steps taken are:
/// - Connect to database
/// - Create the HTTP client that is used to call foreign APIs
/// - Create the route scheme, `/api` for implemented handlers and `/` for Swagger UI
/// - Creates the listener
///
/// # Errors
/// The function returns error if either database connection or creation of HTTP client fails.
pub async fn setup(config: &Config) -> Result<PendingServer, anyhow::Error> {
    let database = database(&config.database_name).await?;

    let http_client = HttpClient::new()?;
    let api = Api::new(http_client, database.clone());

    let api_service =
        OpenApiService::new(api, "Weather Server Demo", "1.0").server("http://localhost:3000/api");
    let ui = api_service.swagger_ui();

    let routes = Route::new().nest("/api", api_service).nest("/", ui);

    let address = format!("0.0.0.0:{}", config.port);
    let listener = TcpListener::bind(address);

    Ok(PendingServer {
        listener,
        routes,
        database,
    })
}

/// Represents a server that is ready to be started.
///
/// Returned by the function `setup`.
///
/// Such an encapsulation is created so tests can start the server at background tasks
/// while the binary can start is normally.
pub struct PendingServer {
    listener: TcpListener<String>,
    routes: Route,
    database: SqlitePool,
}

impl PendingServer {
    /// Starts the server.
    ///
    /// # Errors
    /// Returns error if starting server fails.
    pub async fn serve(self) -> Result<(), std::io::Error> {
        Server::new(self.listener).run(self.routes).await
    }

    #[must_use]
    /// Gives an instance to database connection.
    pub fn database(&self) -> SqlitePool {
        self.database.clone()
    }
}

/// Connects to the database.
///
/// It takes database name, so arbitrary databases can be created by tests and don't cause conflicts.
async fn database(database_name: &str) -> Result<SqlitePool, sqlx::Error> {
    let database_url = format!("sqlite://database/{database_name}.db");

    let database_exists = Sqlite::database_exists(&database_url)
        .await
        .unwrap_or(false);
    if !database_exists {
        Sqlite::create_database(&database_url).await?;
    }

    let database = SqlitePool::connect(&database_url).await?;
    sqlx::migrate!("./migrations").run(&database).await?;

    Ok(database)
}
