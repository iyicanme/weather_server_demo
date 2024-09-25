/*!
Serves a weather information API that locates user from their IP address.

Makes use of `ipapi.co` for geolocation and `weatherapi.com` for weather information.

# Prerequisites
This program requires some configuration over two sources and some setup:

## Configuration file
A configuration file named: `config.toml` is required to be available on program start
in the current working directory.

Configuration file includes two entries:

`port` determines which port the server will serve on. 

`database_name` determines what name the user database file should be.
Database name should not include paths or extensions.

## Environment variables
Program requires two environment variables to be set before start.

`JWT_SECRET` is used as the secret when issuing JWT tokens.

`WEATHER_API_KEY` is the API key for `weatherapi.com`.
An API key can be acquired by signing up at `https://www.weatherapi.com/signup.aspx` and 
heading to `https://www.weatherapi.com/my/`.

These configurations are expected through environment variables so they can be set
when hosted cloud container services through their interfaces.

## Weather API response fields setup
`weatherapi.com` API is configured to send only the required information on API call.

The API can be configured at `https://www.weatherapi.com/my/fields.aspx`.
Under `Current Weather` section, only the fields: `last_updated`, `temp_c`, `text` and 
`feels_like_c` should be selected.

# Running the program
The program is expected to run inside a container described by the provided Dockerfile.
Server itself works with HTTP and HTTPS is mandated through the Docker configuration.

It can be run with:

```bash
docker up -p 443:8000 -e JWT_SECRET=* -e WEATHER_API_KEY=*
```
*/

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
