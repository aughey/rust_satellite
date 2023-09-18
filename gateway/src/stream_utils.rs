use tokio::io::{AsyncRead, AsyncReadExt};

pub async fn receive_length_prefix(
    stream: &mut (impl AsyncRead + Unpin),
    mut buf: Vec<u8>,
) -> std::io::Result<Vec<u8>> {
    // Read the message length (u32)
    let mut length_buffer = [0u8; 4];
    stream.read_exact(&mut length_buffer).await?;
    let length = u32::from_be_bytes(length_buffer);

    // Read the actual message
    buf.resize(length as usize, Default::default());
    stream.read_exact(&mut buf).await?;

    Ok(buf)
}