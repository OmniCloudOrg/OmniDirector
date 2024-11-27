use crate::cpi_actions::{CpiCommand, CpiCommandType};
use debug_print::{
    debug_eprint as deprint, debug_eprintln as deprintln, debug_print as dprint,
    debug_println as dprintln,
};
use ez_logging::println;
use rocket::{self, launch, post, response::Responder, routes, serde::json::Json, State};
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
#[post("/vms/create", format = "json", data = "<params>")]
async fn create(params: Json<CpiCommandType>) -> ApiResult<String> {
    println!("attempted to create vm. received params: {params:?}");
    let cpi = CpiCommand::new()?;
    let result = cpi.execute(params.into_inner())?.to_string();
    
    Ok(Json(result.to_string()))
}

#[post("/vms/delete", format = "json", data = "<params>")]
async fn delete(params: Json<CpiCommandType>) -> ApiResult<String> {
    println!("attempted to delete vm. received params: {params:?}");
    let cpi = CpiCommand::new()?;
    let result = cpi.execute(params.into_inner())?;
    Ok(Json(result.to_string()))
}

#[post("/vms/configure_networks", format = "json", data = "<params>")]
async fn configure_networks(params: Json<CpiCommandType>) -> ApiResult<String> {
    let cpi = CpiCommand::new()?;
    let result = cpi.execute(params.into_inner())?;
    Ok(Json(result.to_string()))
}

#[post("/vms/set_metadata", format = "json", data = "<params>")]
async fn set_metadata(params: Json<CpiCommandType>) -> ApiResult<String> {
    let cpi = CpiCommand::new()?;
    let result = cpi.execute(params.into_inner())?;
    Ok(Json(result.to_string()))
}

#[post("/vms/create_disk", format = "json", data = "<params>")]
async fn create_disk(params: Json<CpiCommandType>) -> ApiResult<String> {
    let cpi = CpiCommand::new()?;
    let result = cpi.execute(params.into_inner())?;
    Ok(Json(result.to_string()))
}

#[post("/vms/attach_disk", format = "json", data = "<params>")]
async fn attach_disk(params: Json<CpiCommandType>) -> ApiResult<String> {
    let cpi = CpiCommand::new()?;
    let result = cpi.execute(params.into_inner())?;
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
            delete,
            configure_networks,
            set_metadata,
            create_disk,
            attach_disk
        ],
    )
}

pub async fn launch_rocket() {
    rocket().await.launch().await.unwrap();
}