use std::sync::Arc;

use clap::Parser;
use rust_satellite::{command_data, Cli, Result};
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    sync::Mutex,
};
use tracing::{info, trace};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args = Cli::parse();

    info!("Connecting to {}:{}", args.host, args.port);

    // open up an async tcp connection to the host and port
    // and send a message
    let stream = tokio::net::TcpStream::connect((args.host.as_str(), args.port)).await?;
    info!("Connected to {}:{}", args.host, args.port);

    // turn stream into a lines stream
    let stream = tokio::io::BufStream::new(stream);
    let (reader, mut writer) = tokio::io::split(stream);

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
        let (command, data) =
            command_data(&line).ok_or_else(|| anyhow::anyhow!("Bad command: {}", line))?;

        match command {
            "PONG" => {
                trace!("Received PONG");
            }
            "BEGIN" => {
                info!("Beginning communication: {data}");
            }
            _ => {
                info!("Unknown command: {}", command);
            }
        }
    }

    Ok(())
}
