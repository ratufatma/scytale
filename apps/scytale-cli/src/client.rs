//! IPC client implementation with fail-closed graceful error handling.

use std::path::Path;
use tokio::net::UnixStream;

use scytale_bridge::{read_ipc_message, write_ipc_message, NodeRequest, NodeResponse};

#[derive(Debug, thiserror::Error)]
pub enum CliClientError {
    #[error("Error: Node daemon is not running. Start scytale-node first.")]
    DaemonNotRunning,
    #[error("Daemon disconnected before responding")]
    Disconnected,
    #[error("IPC communication error: {0}")]
    Bridge(#[from] scytale_bridge::BridgeError),
    #[error("Identity error: {0}")]
    Identity(#[from] crate::identity::IdentityError),
    #[error("Wallet error: {0}")]
    Wallet(#[from] crate::wallet::WalletError),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    User(String),
}

/// Sends a request to the running node daemon over a Unix domain socket and waits for response.
pub async fn send_node_request(
    socket_path: impl AsRef<Path>,
    request: NodeRequest,
) -> Result<NodeResponse, CliClientError> {
    let path = socket_path.as_ref();
    let stream = match UnixStream::connect(path).await {
        Ok(s) => s,
        Err(e)
            if e.kind() == std::io::ErrorKind::NotFound
                || e.kind() == std::io::ErrorKind::ConnectionRefused =>
        {
            return Err(CliClientError::DaemonNotRunning);
        }
        Err(e) => return Err(CliClientError::Io(e)),
    };

    let (reader, mut writer) = stream.into_split();
    let mut buf_reader = tokio::io::BufReader::new(reader);

    write_ipc_message(&mut writer, &request).await?;
    let response: Option<NodeResponse> = read_ipc_message(&mut buf_reader).await?;

    response.ok_or(CliClientError::Disconnected)
}
