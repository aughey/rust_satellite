use std::sync::Arc;

use clap::Parser;
use rust_satellite::{Cli, Result};
use streamdeck::StreamDeck;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};
use tracing::{info, trace};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    // Connect to device
    let mut deck =  StreamDeck::connect(0x0fd9, 0x0063, None)?;

    let serial = deck.serial().unwrap();
    info!(
        "Connected to device {}", serial
    );

    info!("Connecting to {}:{}", args.host, args.port);

    // open up an async tcp connection to the host and port
    // and send a message
    let stream = tokio::net::TcpStream::connect((args.host.as_str(), args.port)).await?;
    info!("Connected to {}:{}", args.host, args.port);

    // turn stream into a lines stream
    let stream = tokio::io::BufStream::new(stream);
    let (reader, writer) = tokio::io::split(stream);

    let writer = Arc::new(Mutex::new(writer));

    // ping task
    {
        let writer = writer.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                let mut writer = writer.lock().await;
                writer.write_all(b"PING\n").await.unwrap();
                writer.flush().await.unwrap();
            }
        });
    }

    // tell it there is a device
    {
        let mut writer = writer.lock().await;
        writer
            .write_all(
                format!(
                    "ADD-DEVICE {}\n",
                    rust_satellite::DeviceMsg {
                        device_id: "JohnAughey".to_string(),
                        product_name: "Satellite StreamDeck: plus".to_string(),
                        keys_total: 16,
                        keys_per_row: 4,
                    }
                    .device_msg()
                )
                .as_bytes(),
            )
            .await?;
    }

    let mut lines = BufReader::new(reader).lines();

    while let Some(line) = lines.next_line().await? {
        let command = rust_satellite::Command::parse(&line)
            .map_err(|e| anyhow::anyhow!("Error parsing line: {}, {:?}", line, e))?;

        match command {
            rust_satellite::Command::Pong => {
                trace!("Received PONG");
            }
            rust_satellite::Command::Begin(versions) => {
                info!("Beginning communication: {:?}", versions);
            }
            rust_satellite::Command::AddDevice(device) => {
                info!("Adding device: {:?}", device);
            }
            rust_satellite::Command::KeyState(keystate) => {
                info!("Received key state: {:?}", keystate);
                info!("  bitmap size: {}", keystate.bitmap()?.len());
            }
            rust_satellite::Command::Brightness(brightness) => {
                info!("Received brightness: {:?}", brightness);
            }
            rust_satellite::Command::Unknown(command) => {
                info!("Unknown command: {} with data {}", command, line.len());
            }
        }
    }

    Ok(())
}
