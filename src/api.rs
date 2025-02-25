use super::cpi_actions::actions::CpiAction;
use ez_logging::println;
use rocket::{self, post, response::Responder, routes, serde::json::Json};
use std::env;

// Custom error handling
#[derive(Debug, Responder)]
enum ApiError {
    #[response(status = 500)]
    Internal(String),
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

// Route handlers
#[post("/vms/action", format = "json", data = "<params>")]
async fn create(params: Json<CpiAction>) -> ApiResult<String> {
    let command = params.into_inner();
    println!("received params: {:#?}", command);
    let result = command.execute().unwrap();

    Ok(Json(result.to_string()))
}

pub async fn rocket() -> rocket::Rocket<rocket::Build> {
    // Load environment variables
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());

    println!("Server running at http://{}:{}", host, &port);

    // Configure Rocket
    let config = rocket::Config::figment()
        .merge(("address", host))
        .merge(("port", port.parse::<u16>().unwrap()));

    rocket::custom(config).mount(
        "/",
        routes![
            create,
        ],
    )
}

pub async fn launch_rocket() {
    rocket().await.launch().await.unwrap();
}