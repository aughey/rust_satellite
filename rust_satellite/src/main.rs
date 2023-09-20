use clap::Parser;
use rust_satellite::{Cli, Result};

use tracing::info;
use traits::device::Receiver;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    info!("Starting native satellite application");

    let mut streamdeck = streamdeck::StreamDeck::open().await?;
    let first_msg = streamdeck.0.receive().await?.as_config()?;

    pumps::run_satellite(
        move || {
            let streamdeck = streamdeck.clone();
            async move { Ok(streamdeck) }
        },
        move |_| {
            let hostport = (args.host.clone(), args.port);
            let first_msg = first_msg.clone();
            async {
                info!("Connecting to companion: {}:{}", hostport.0, hostport.1);
                companion::connect(hostport, first_msg).await
            }
        },
    )
    .await?;

    Ok(())
}
