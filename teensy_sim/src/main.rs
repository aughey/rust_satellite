use anyhow::Result;
use hidapi::HidApi;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, AsyncReadExt};

pub const ELGATO_VENDOR_ID: u16 = 0x0fd9;
pub const PID_STREAMDECK_MK2: u16 = 0x0080;
pub const SERIAL: u16 = 0x0001;

#[tokio::main]
async fn main() -> Result<()> {
    let hidapi = HidApi::new()?;
    let mut devices = hidapi.device_list().filter_map(|d| {
        if d.vendor_id() != ELGATO_VENDOR_ID {
            return None;
        }

        if let Some(serial) = d.serial_number() {
            if !serial.chars().all(|c| c.is_alphanumeric()) {
                return None;
            }

            Some((d.product_id(), serial.to_string()))
        } else {
            None
        }
    });
    // for device in devices {
    //     println!("{:?}", device);
    // }
    let first_dev = devices
        .next()
        .ok_or_else(|| anyhow::anyhow!("No matching devices found"))?;

    let device = hidapi.open_serial(ELGATO_VENDOR_ID, first_dev.0, first_dev.1.as_str())?;

    println!("Opened device");

    // create a tcp socket listen on 12345
    let socket = tokio::net::TcpListener::bind("0.0.0.0:12345").await?;

    loop {
        println!("Waiting...");
        // wait for a connection
        let (stream, _) = socket.accept().await?;
        println!("Got connection");

        // Split the stream innto buffered read and write
        let (reader, mut writer) = tokio::io::split(stream);

        // create a buffered stream
        let mut bufstream = tokio::io::BufReader::new(reader);
        loop {
            // Read next line
            let mut line = String::new();
            bufstream.read_line(&mut line).await?;

            println!("Got line: {}", line);
            let mut line = line.trim().split(' ');
            let command = line.next();
            match command {
                Some("tryread") => {
                    let size = line.next().unwrap().parse::<usize>()?;
                    println!("Try read: {}", size);
                    let mut buf = vec![0; size];
                    let bytes_read = device.read_timeout(&mut buf, 0)?;
                    // must read that many bytes
                    writer.write_all(format!("0\n").as_bytes()).await?;
                    // resize buf
                    println!("Read from device: {bytes_read}");
                    // Write the response back to the client
                    writer.write_all(&buf[..bytes_read]).await?;
                }
                Some("write") => {
                    let size = line.next().unwrap().parse::<usize>()?;
                    println!("Write: {}", size);
                    let mut buf = vec![0; size];
                    let bytes_read = bufstream.read_exact(&mut buf).await?;
                    if bytes_read != size {
                        println!("Error: write {} bytes, expected {}", bytes_read, size);
                        break;
                    }
                    let bytes_wrote = device.write(&buf)?;
                    if bytes_wrote != size {
                        println!("Error: read {} bytes, expected {}", bytes_read, size);
                        break;
                    }
                    println!("writing to device from device: {bytes_read}");
                    writer.write_all(format!("OK\n").as_bytes()).await?;
                }
                Some("read") => {
                    let size = line.next().unwrap().parse::<usize>()?;
                    let mut buf = vec![0; size];
                    let bytes_read = device.read(&mut buf)?;
                    if bytes_read != size {
                        println!("Error: read {} bytes, expected {}", bytes_read, size);
                        break;
                    }
                    // Write the response back to the client
                    writer.write_all(&buf).await?;
                }
                Some("get_feature_report") => {
                    let report = line.next().unwrap().parse::<usize>()?;
                    let size = line.next().unwrap().parse::<usize>()?;
                    println!("Get feature report: {}", size);
                    let mut buf = vec![0; size];
                    buf[0] = report as u8;
                    println!("Getting");
                    let bytes_read = device.get_feature_report(&mut buf)?;
                    println!("Got {} bytes", bytes_read);
                    if bytes_read != size {
                        println!("Error: read {} bytes, expected {}", bytes_read, size);
                        break;
                    }
                    println!("writing to device from device: {bytes_read}");
                    // Write the response back to the client
                    writer.write_all(&buf).await?;
                }
                Some("send_feature_report") => {
                    let size = line.next().unwrap();
                    println!("looking at size #{size}#");
                    let size = size.parse::<usize>().expect("Valid integer");
                    println!("  Got size {}", size);
                    let mut buf = vec![0; size];
                    let bytes_read = bufstream.read_exact(&mut buf).await?;
                    if bytes_read != size {
                        println!("Error: read {} bytes, expected {}", bytes_read, size);
                        break;
                    }
                    println!("  writing to device: {}",buf.len());
                    device.send_feature_report(&buf[0..bytes_read])?;
                    writer.write_all(format!("OK\n").as_bytes()).await?;
                }
                _ => {
                    println!("Unknown command");
                    break;
                }
            }
            println!("DONE!");
        }
    }

    #[allow(unreachable_code)]
    Ok(())
}
