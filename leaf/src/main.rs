use leaf::Result;
use clap::Parser;
use leaf::Cli;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    pumps::create_and_run(streamdeck::StreamDeck::open_first, move |_| {
        let hostport = (args.gateway_host.clone(), args.gateway_port);
        async {
            info!("Connecting to gateway: {}:{}", hostport.0, hostport.1);
            let (leaf_sender, leaf_receiver) = gateway_devices::connect_to_gateway(hostport).await?;
            info!("Connected to gateway");
            Ok((leaf_sender, leaf_receiver))
        }
    })
    .await?;

    Ok(())
}
