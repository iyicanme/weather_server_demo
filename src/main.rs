use weather_server_lib::config::Config;
use weather_server_lib::server;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    if std::env::var_os("RUST_LOG").is_none() {
        unsafe {
            std::env::set_var("RUST_LOG", "poem=debug");
        }
    }

    tracing_subscriber::fmt::init();

    let config = Config::read().expect("could not read config");

    let server = server::setup(&config)
        .await
        .expect("server initialization failed");
    server.serve().await.expect("server execution interrupted");
}
