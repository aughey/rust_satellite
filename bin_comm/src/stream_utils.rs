use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::io::{AsyncWrite, AsyncWriteExt};

/// Read a message from the stream, prefixed with a u32 length.
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

/// Serialize a serde value using bincode and write it to a stream
/// using a length prefix.
pub async fn write_struct(
    stream: &mut (impl AsyncWrite + Unpin),
    data: &impl serde::Serialize,
) -> std::io::Result<()> {
    let buf = bincode::serialize(data).unwrap();
    write_length_prefix(stream, buf).await
}

/// Write a message to the stream, prefixed with a u32 length.
pub async fn write_length_prefix(
    stream: &mut (impl AsyncWrite + Unpin),
    buf: impl AsRef<[u8]>,
) -> std::io::Result<()> {
    let buf = buf.as_ref();

    // Write the message length (u32)
    let length = buf.len() as u32;
    stream.write_all(&length.to_be_bytes()).await?;

    // Write the actual message
    stream.write_all(buf).await?;
    stream.flush().await?;
    Ok(())
}

/// Read a struct from a stream that is prefixed with a u32 length deserialized
/// using bincode and serde.
pub async fn read_struct<T>(stream: &mut (impl AsyncRead + Unpin)) -> anyhow::Result<T>
where
    T: serde::de::DeserializeOwned,
{
    let buf = receive_length_prefix(stream, Vec::new()).await?;
    let data = bincode::deserialize(&buf)?;
    Ok(data)
}
