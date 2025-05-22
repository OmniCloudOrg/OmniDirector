mod api;
mod cpis;
mod logging;

pub mod proposal;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize the CPI system
    println!("Initializing CPI system...");
    let cpi_system = cpis::initialize();

    // List available providers
    // TODO: @tristanpoland - Uncomment this when the cpi_system is implemented
    println!("Available providers:");
    // for provider in cpi_system.get_providers() {
    //     println!("  - {}", provider);
    // }

    //let input_dir: &str = "./";
    //try_compile(input_dir).expect("Could not compile");
    api::launch_rocket().await;
    Ok(())
}
