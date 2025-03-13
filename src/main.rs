mod logging;
mod cpis;
mod api;

pub mod proposal;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()>{
    ez_logging::init()?;

    // Initialize the CPI system
    let cpi_system = cpis::initialize()?;
    
    // List available providers
    println!("Available providers:");
    for provider in cpi_system.get_providers() {
        println!("  - {}", provider);
    }


    //let input_dir: &str = "./";
    //try_compile(input_dir).expect("Could not compile");
    api::launch_rocket().await;
    Ok(())
}