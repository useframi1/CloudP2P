use clap::Parser;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;

mod election;
mod messages;
mod server;

use server::{Server, ServerConfig};

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Configuration file path
    #[arg(short, long)]
    config: String,
}

fn init_logger() {
    Builder::new()
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] [{}] {}",
                chrono::Local::now().format("%H:%M:%S"),
                record.level(),
                record.args()
            )
        })
        .filter_level(LevelFilter::Info)
        .init();
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_logger();

    let args = Args::parse();

    let config = ServerConfig::from_file(&args.config)?;
    let server = Server::new(config);

    server.run().await;

    Ok(())
}
