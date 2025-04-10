use crate::cpis::{self, error::CpiError};
use ez_logging::println;
use rocket::{self, get, post, response::Responder, routes, serde::json::Json};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, sync::Arc};

// CPI System instance stored in application state
struct CpiState {
    cpi_system: Arc<cpis::CpiSystem>,
}

// Request format for CPI actions
#[derive(Debug, Deserialize)]
struct CpiActionRequest {
    provider: String,
    action: String,
    #[serde(default)]
    params: HashMap<String, serde_json::Value>,
}

// Response format for CPI actions
#[derive(Debug, Serialize)]
struct CpiActionResponse {
    success: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

// Custom error handling
#[derive(Debug, Responder)]
enum ApiError {
    #[response(status = 400)]
    BadRequest(String),

    #[response(status = 404)]
    NotFound(String),

    #[response(status = 500)]
    Internal(String),
}

impl From<CpiError> for ApiError {
    fn from(err: CpiError) -> Self {
        match err {
            CpiError::ProviderNotFound(name) => {
                ApiError::NotFound(format!("Provider not found: {}", name))
            }
            CpiError::ActionNotFound(name) => {
                ApiError::NotFound(format!("Action not found: {}", name))
            }
            CpiError::MissingParameter(name) => {
                ApiError::BadRequest(format!("Missing required parameter: {}", name))
            }
            CpiError::InvalidParameterType(name, expected) => ApiError::BadRequest(format!(
                "Invalid parameter type for {}, expected {}",
                name, expected
            )),
            _ => ApiError::Internal(err.to_string()),
        }
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err.to_string())
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

// Route handlers
#[post("/action", format = "json", data = "<action_request>")]
async fn execute_action(
    action_request: Json<CpiActionRequest>,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<CpiActionResponse> {
    let request = action_request.into_inner();
    println!("Received action request: {:#?}", request);

    // Execute the CPI action
    let result =
        match cpi_state
            .cpi_system
            .execute(&request.provider, &request.action, request.params)
        {
            Ok(value) => CpiActionResponse {
                success: true,
                result: Some(value),
                error: None,
            },
            Err(err) => CpiActionResponse {
                success: false,
                result: None,
                error: Some(err.to_string()),
            },
        };

    Ok(Json(result))
}

// Get available providers
#[get("/providers")]
async fn get_providers(cpi_state: &rocket::State<CpiState>) -> ApiResult<Vec<String>> {
    let providers = cpi_state.cpi_system.get_providers();
    Ok(Json(providers))
}

// Get available actions for a provider
#[get("/actions/<provider>")]
async fn get_actions(
    provider: String,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<Vec<String>> {
    let actions = cpi_state.cpi_system.get_provider_actions(&provider)?;
    Ok(Json(actions))
}

// Get all unique actions across all providers
#[get("/actions")]
async fn get_all_unique_actions(cpi_state: &rocket::State<CpiState>) -> ApiResult<Vec<String>> {
    let providers = cpi_state.cpi_system.get_providers();
    let mut all_actions = Vec::new();

    // Collect actions from all providers
    for provider in providers {
        match cpi_state.cpi_system.get_provider_actions(&provider) {
            Ok(provider_actions) => {
                all_actions.extend(provider_actions);
            }
            Err(err) => {
                println!("Error getting actions for provider {}: {}", provider, err);
                // Continue with other providers even if one fails
            }
        }
    }

    // Remove duplicates by using a HashSet
    let unique_actions: Vec<String> = all_actions
        .into_iter()
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();

    Ok(Json(unique_actions))
}

// Get required parameters for an action
#[get("/params/<provider>/<action>")]
async fn get_action_params(
    provider: String,
    action: String,
    cpi_state: &rocket::State<CpiState>,
) -> ApiResult<Vec<String>> {
    let params = cpi_state.cpi_system.get_action_params(&provider, &action)?;
    Ok(Json(params))
}

pub async fn rocket() -> rocket::Rocket<rocket::Build> {
    // Load environment variables
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());

    println!("Server running at http://{}:{}", host, &port);

    // Initialize CPI system
    let cpi_system = match cpis::initialize() {
        Ok(system) => system,
        Err(err) => {
            println!("Failed to initialize CPI system: {}", err);
            std::process::exit(1);
        }
    };

    // Log loaded providers
    let providers = cpi_system.get_providers();
    println!("Loaded CPI providers: {:?}", providers);

    // Configure Rocket
    let config = rocket::Config::figment()
        .merge(("address", host))
        .merge(("port", port.parse::<u16>().unwrap()));

    rocket::custom(config)
        .manage(CpiState {
            cpi_system: Arc::new(cpi_system),
        })
        .mount(
            "/",
            routes![
                execute_action,
                get_providers,
                get_actions,
                get_action_params,
                get_all_unique_actions,
            ],
        )
}

pub async fn launch_rocket() {
    rocket().await.launch().await.unwrap();
}
