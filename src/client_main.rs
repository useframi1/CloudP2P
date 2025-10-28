use clap::Parser;
use env_logger::Builder;
use log::LevelFilter;
use std::io::Write;

mod clients;
mod election;
mod messages;
mod server;
mod steganography;

use clients::{Client, ClientConfig};

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
    let config = ClientConfig::from_file(&args.config)?;

    let mut client = Client::new(config);
    client.run().await;

    Ok(())
}
