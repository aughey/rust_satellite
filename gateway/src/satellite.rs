use bin_comm::RemoteCommands;
use tokio::io::AsyncRead;
use crate::Result;
use crate::stream_utils::receive_length_prefix;

pub async fn satellite_read_command(stream: &mut (impl AsyncRead + Unpin)) -> Result<RemoteCommands> {
    let buf = receive_length_prefix(stream, Default::default()).await?;
    // bincode decode it
    Ok(bincode::deserialize(&buf)?)
}