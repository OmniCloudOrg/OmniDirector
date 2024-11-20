use crate::cpi_actions::{CpiCommand, CpiCommandType};
use debug_print::{
    debug_eprint as deprint, debug_eprintln as deprintln, debug_print as dprint,
    debug_println as dprintln,
};
use ez_logging::println;
use std::env;
use tokio;
use warp::reject::Reject;
use warp::Filter;
#[derive(Debug)]
struct CustomError(anyhow::Error);

impl Reject for CustomError {}

#[tokio::main]
pub async fn main() {
    // Load environment variables

    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "8080".to_string());

    println!("Server running at http://{}:{}", host, &port);

    // Combine all routes
    let routes = warp::path("vms").and(
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
                        Err(err) => Err(warp::reject::custom(CustomError(err))),
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
                        Err(err) => Err(warp::reject::custom(CustomError(err))),
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
                        Err(err) => Err(warp::reject::custom(CustomError(err))),
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
                        Err(err) => Err(warp::reject::custom(CustomError(err))),
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
                        Err(err) => Err(warp::reject::custom(CustomError(err))),
                    }
                })),
    );

    warp::serve(routes)
        .run((
            host.parse::<std::net::IpAddr>().unwrap(),
            port.parse::<u16>().unwrap(),
        ))
        .await;
}

fn auth() -> impl warp::Filter<Extract = ((),), Error = std::convert::Infallible> + Clone {
    warp::any().map(|| ())
}

struct AuthToken;

impl AuthToken {
    fn new() -> Self {
        AuthToken {}
    }
    fn new_preset(expected_token: &str) -> Self {
        AuthToken::new()
    }
}
