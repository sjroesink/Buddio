use rmcp::{transport::stdio, ServiceExt};

mod server;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse optional --db <path> from command-line arguments
    let db_path = {
        let args: Vec<String> = std::env::args().collect();
        args.windows(2)
            .find(|w| w[0] == "--db")
            .map(|w| std::path::PathBuf::from(&w[1]))
    };

    let server = server::GoLaunchMcp::new(db_path);
    let service = server.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
