use std::env;
use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::broadcast;
use tokio_tungstenite::{accept_async, tungstenite::Message};

#[tokio::main]
async fn main() {
    let bind = env::var("VEIL_RELAY_BIND").unwrap_or_else(|_| "127.0.0.1:9001".to_string());
    let addr: SocketAddr = bind.parse().expect("invalid VEIL_RELAY_BIND");

    let listener = TcpListener::bind(addr)
        .await
        .expect("failed to bind relay socket");

    let (tx, _rx) = broadcast::channel::<Vec<u8>>(1024);
    let tx = Arc::new(tx);

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(value) => value,
            Err(_) => continue,
        };
        let relay_tx = Arc::clone(&tx);
        tokio::spawn(async move {
            if let Err(err) = handle_client(stream, relay_tx).await {
                eprintln!("client error: {err}");
            }
        });
    }
}

async fn handle_client(
    stream: TcpStream,
    relay_tx: Arc<broadcast::Sender<Vec<u8>>>,
) -> anyhow::Result<()> {
    let ws_stream = accept_async(stream).await?;
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let mut rx = relay_tx.subscribe();

    let forward_in = async {
        while let Some(msg) = ws_receiver.next().await {
            let msg = msg?;
            if let Message::Binary(bytes) = msg {
                let _ = relay_tx.send(bytes);
            }
        }
        Ok::<(), anyhow::Error>(())
    };

    let forward_out = async {
        while let Ok(bytes) = rx.recv().await {
            ws_sender.send(Message::Binary(bytes)).await?;
        }
        Ok::<(), anyhow::Error>(())
    };

    tokio::select! {
        res = forward_in => res?,
        res = forward_out => res?,
    }

    Ok(())
}
