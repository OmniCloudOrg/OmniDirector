mod logging;
mod cpi_actions;
mod api;
pub mod proposal;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()>{
    ez_logging::init()?;
    cpi_actions::test();

    //let input_dir: &str = "./";
    //try_compile(input_dir).expect("Could not compile");
    api::launch_rocket().await;
    Ok(())
}