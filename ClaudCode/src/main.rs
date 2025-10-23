use clap::Parser;
use cloud_p2p_image_sharing::Server;
use cloud_p2p_image_sharing::server::ServerConfig;
use cloud_p2p_image_sharing::utils::logging;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long)]
    config: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    logging::init_logger();
    
    let args = Args::parse();
    
    let config = ServerConfig::from_file(&args.config)?;
    let server = Server::new(config);
    
    server.run().await;
    
    Ok(())
}