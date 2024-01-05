use clap::Parser;
use rust_satellite::{Cli, Result};

use tracing::info;
use traits::device::Receiver;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    info!("Starting native satellite application");

    let mut streamdeck = streamdeck::StreamDeck::open_first().await?;
    let first_msg = streamdeck.0.receive().await?;
    let first_msg = match first_msg {
        traits::device::Command::Config(c) => traits::device::RemoteConfig {
            pid: c.pid.try_into()?,
            device_id: c.device_id,
        },
        _ => anyhow::bail!("Expected config msg to be first"),
    };

    pumps::create_and_run(
        move || {
            let streamdeck = streamdeck.clone();
            async move { Ok(streamdeck) }
        },
        move |_| {
            let hostport = (args.companion_host.clone(), args.companion_port);
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
