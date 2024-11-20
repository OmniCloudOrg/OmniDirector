use actix_web::{App, HttpServer, middleware};
use dotenv::dotenv;
use std::env;

mod routes;
mod handlers;
mod models;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Load environment variables
    dotenv().ok();
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    println!("Server running at http://{}:{}", host, &port);

    HttpServer::new(|| {
        App::new()
            .wrap(middleware::Logger::default())
            // Register routes
            .configure(routes::users::config)
            .configure(routes::posts::config)
            .configure(routes::health::config)
    })
    .bind(format!("{}:{}", host, port))?
    .run()
    .await
}