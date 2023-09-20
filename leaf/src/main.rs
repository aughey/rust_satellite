use anyhow::Result;
use clap::Parser;
use leaf::Cli;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    pumps::run_satellite(streamdeck::StreamDeck::open, move |_| {
        let hostport = (args.host.clone(), args.port);
        async {
            info!("Connecting to gateway: {}:{}", hostport.0, hostport.1);
            let (leaf_sender, leaf_receiver) = gateway_devices::connect(hostport).await?;
            info!("Connected to gateway");
            Ok((leaf_sender, leaf_receiver))
        }
    })
    .await?;

    Ok(())
}
