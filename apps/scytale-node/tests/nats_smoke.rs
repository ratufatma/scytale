use futures::{SinkExt, StreamExt};
use std::net::TcpStream;
use std::time::Duration;

fn nats_broker_online() -> bool {
    let addrs = ["116.212.72.89:4222", "127.0.0.1:4222"];
    for addr in addrs {
        if TcpStream::connect_timeout(&addr.parse().unwrap(), Duration::from_secs(1)).is_ok() {
            return true;
        }
    }
    false
}

#[tokio::test]
#[ignore = "memerlukan broker NATS aktif"]
async fn nats_smoke_test() -> Result<(), Box<dyn std::error::Error>> {
    if !nats_broker_online() {
        eprintln!("Broker NATS offline, melewati smoke test.");
        return Ok(());
    }

    let mut client = async_nats::ConnectOptions::new()
        .name("scytale-nats-smoke")
        .connect("nats://116.212.72.89:4222")
        .await?;

    let subject = "scytale.v1.nodes.heartbeat";
    let mut sub = client.subscribe(subject.to_string()).await?;
    client
        .publish(subject, b"smoke-test".to_vec().into())
        .await?;
    client.flush().await?;

    match tokio::time::timeout(Duration::from_secs(3), sub.next()).await {
        Ok(Some(message)) => {
            assert_eq!(message.payload.as_ref(), b"smoke-test");
        }
        Ok(None) => panic!("NATS subscription closed unexpectedly"),
        Err(_) => panic!("timed out waiting for NATS message"),
    }

    client.close().await?;
    Ok(())
}
