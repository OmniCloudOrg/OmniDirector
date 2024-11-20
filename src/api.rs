
use std::env;
use warp::Filter;
use tokio;
use crate::cpi_actions::{CpiCommand, CpiCommandType};
use warp::reject::Reject;

#[derive(Debug)]
struct CustomError(anyhow::Error);

impl Reject for CustomError {}

#[tokio::main]
async fn main() {
    // Load environment variables

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    println!("Server running at http://{}:{}", host, &port);

    // Combine all routes
    let routes = 
    warp::path("vms")
        .and(
            warp::post()
                .and(warp::path("create"))
                .and(warp::body::json())
                .and_then(|params: CpiCommandType| async move {
                    let cpi = CpiCommand::new().unwrap();
                    let result = cpi.execute(params);
                    match result {
                        Ok(output) => Ok(warp::reply::json(&output)),
                        Err(err) => Err(warp::reject::custom(CustomError(err))),
                    }
                })
                .or(warp::post()
                    .and(warp::path("delete"))
                    .and(warp::body::json())
                    .and_then(|params: CpiCommandType| async move {
                        let cpi = CpiCommand::new().unwrap();
                        let result = cpi.execute(params);
                        match result {
                            Ok(output) => Ok(warp::reply::json(&output)),
                            Err(err) => Err(warp::reject::custom(err)),
                        }
                    }))
                .or(warp::post()
                    .and(warp::path("configure_networks"))
                    .and(warp::body::json())
                    .and_then(|params: CpiCommandType| async move {
                        let cpi = CpiCommand::new().unwrap();
                        let result = cpi.execute(params);
                        match result {
                            Ok(output) => Ok(warp::reply::json(&output)),
                            Err(err) => Err(warp::reject::custom(err)),
                        }
                    }))
                .or(warp::post()
                    .and(warp::path("set_metadata"))
                    .and(warp::body::json())
                    .and_then(|params: CpiCommandType| async move {
                        let cpi = CpiCommand::new().unwrap();
                        let result = cpi.execute(params);
                        match result {
                            Ok(output) => Ok(warp::reply::json(&output)),
                            Err(err) => Err(warp::reject::custom(err)),
                        }
                    }))
                .or(warp::post()
                    .and(warp::path("create_disk"))
                    .and(warp::body::json())
                    .and_then(|params: CpiCommandType| async move {
                        let cpi = CpiCommand::new().unwrap();
                        let result = cpi.execute(params);
                        match result {
                            Ok(output) => Ok(warp::reply::json(&output)),
                            Err(err) => Err(warp::reject::custom(err)),
                        }
                    }))
                .or(warp::post()
                    .and(warp::path("attach_disk"))
                    .and(warp::body::json())
                    .and_then(|params: CpiCommandType| async move {
                        let cpi = CpiCommand::new().unwrap();
                        let result = cpi.execute(params);
                        match result {
                            Ok(output) => Ok(warp::reply::json(&output)),
                            Err(err) => Err(warp::reject::custom(err)),
                        }
                    }))
                );

    warp::serve(routes)
        .run((host.parse().unwrap(), port.parse().unwrap()))
        .await;
}

fn auth() -> impl warp::Filter<Extract = (),Error = warp::Rejection> + Clone {
    
}

struct AuthToken(String);
impl AuthToken {
    fn new() -> Self {
        AuthToken { };
    }
    fn new_preset(expected_token: &str) -> Self {
        AuthToken( )
    }
}