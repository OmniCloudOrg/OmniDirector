use warp::Filter;

pub async fn init() {
    // GET /hello -> returns a greeting
    let hello = warp::path("hello")
        .and(warp::get())
        .map(|| warp::reply::json(&"Hello, World!"));

    // POST /echo -> returns the posted body
    let echo = warp::path("echo")
        .and(warp::post())
        .and(warp::body::json())
        .map(|body: serde_json::Value| warp::reply::json(&body));

    // Combine the routes
    let routes = hello.or(echo);

    // Start the server
    warp::serve(routes).run(([127, 0, 0, 1], 3030)).await;
}