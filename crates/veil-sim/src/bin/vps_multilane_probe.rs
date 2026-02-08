use std::env;
use std::io::Read;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rand::rngs::OsRng;
use rand::RngCore;
use base64::engine::general_purpose::STANDARD as Base64Standard;
use base64::Engine as _;
use veil_codec::shard::{encode_shard_cbor, ShardHeaderV1, ShardV1, SHARD_HEADER_LEN};
use veil_core::{Epoch, Namespace, Tag};
use veil_transport::adapter::TransportAdapter;
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};
use veil_transport_websocket::{WebSocketAdapter, WebSocketAdapterConfig};

const DEFAULT_CORE_TAGS: [&str; 4] = [
    "6914e6d3b151b9ac372db7c201ae4e043af645245ecce6175648d42b6177a9ca",
    "7f3612b9145b9ae924e119dbce48ea5bba8ef366d50f10fdf490fc88378c7180",
    "040257d0dadd0ec43e267cc60c2a3c4306e1665273e0ba88065254bbd082a590",
    "7f3fccfbad7a618eecccf31277a79691c5d6a657e50f45dd671319f84ee1d010",
];

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        print_usage();
        return;
    }

    let domain = get_arg_value(&args, "--domain");
    let mut ws_url = get_arg_value(&args, "--ws");
    let mut quic_peer = get_arg_value(&args, "--quic");
    let quic_cert_hex = get_arg_value(&args, "--quic-cert-hex");
    let quic_cert_b64 = get_arg_value(&args, "--quic-cert-b64");
    let quic_cert_path = get_arg_value(&args, "--quic-cert-path");
    let mut quic_cert_url = get_arg_value(&args, "--quic-cert-url");
    let tag_hex = get_arg_value(&args, "--tag")
        .or_else(|| env::var("VEIL_VPS_CORE_TAGS").ok().and_then(|v| v.split(',').next().map(|s| s.to_string())))
        .unwrap_or_else(|| DEFAULT_CORE_TAGS[0].to_string());
    let namespace = get_arg_value(&args, "--namespace")
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(1);
    let timeout_secs = get_arg_value(&args, "--timeout")
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(12);

    if let Some(host) = domain.as_deref() {
        if ws_url.is_none() {
            ws_url = Some(format!("wss://{host}/ws"));
        }
        if quic_peer.is_none() {
            quic_peer = Some(format!("{host}:5000"));
        }
        if quic_cert_url.is_none() {
            quic_cert_url = Some(format!("https://{host}/veil/quic_cert.der"));
        }
    }

    if ws_url.is_none() && quic_peer.is_none() {
        eprintln!(
            "Provide at least one lane: --domain <host> or --ws <wss://.../ws> or --quic <host:port>"
        );
        print_usage();
        std::process::exit(2);
    }

    let tag = match parse_hex_tag(&tag_hex) {
        Ok(tag) => tag,
        Err(err) => {
            eprintln!("Invalid tag hex: {err}");
            std::process::exit(2);
        }
    };

    let epoch = current_epoch();
    let mut object_root_ws = [0u8; 32];
    let mut object_root_ws_warm = [0u8; 32];
    let mut object_root_quic = [0u8; 32];
    let mut object_root_quic_warm = [0u8; 32];
    OsRng.fill_bytes(&mut object_root_ws);
    OsRng.fill_bytes(&mut object_root_ws_warm);
    OsRng.fill_bytes(&mut object_root_quic);
    OsRng.fill_bytes(&mut object_root_quic_warm);

    let ws_shard = build_shard(tag, Namespace(namespace), epoch, object_root_ws, 0);
    let ws_warmup = build_shard(tag, Namespace(namespace), epoch, object_root_ws_warm, 1);
    let quic_shard = build_shard(tag, Namespace(namespace), epoch, object_root_quic, 0);
    let quic_warmup = build_shard(tag, Namespace(namespace), epoch, object_root_quic_warm, 1);

    println!("== VEIL VPS multilane probe ==");
    println!("Tag: {tag_hex}");
    println!("Namespace: {namespace}");
    println!("Epoch: {}", epoch.0);
    if let Some(ws) = &ws_url {
        println!("WS: {ws}");
    }
    if let Some(quic) = &quic_peer {
        println!("QUIC: {quic}");
    }

    let mut ws_sender = ws_url
        .as_ref()
        .map(|url| build_ws_adapter(url, "ws-probe-a"));
    let mut ws_receiver = ws_url
        .as_ref()
        .map(|url| build_ws_adapter(url, "ws-probe-b"));

    let mut quic_sender = None;
    let mut quic_receiver = None;
    if let Some(peer) = quic_peer.as_ref() {
        match build_quic_adapter(
            peer,
            quic_cert_hex.clone(),
            quic_cert_b64.clone(),
            quic_cert_path.clone(),
            quic_cert_url.clone(),
        ) {
            Ok(adapter) => quic_sender = Some(adapter),
            Err(err) => eprintln!("QUIC sender error: {err}"),
        }
        match build_quic_adapter(peer, quic_cert_hex, quic_cert_b64, quic_cert_path, quic_cert_url)
        {
            Ok(adapter) => quic_receiver = Some(adapter),
            Err(err) => eprintln!("QUIC receiver error: {err}"),
        }
    }

    wait_for_ws_ready(ws_sender.as_mut(), ws_receiver.as_mut());

    let mut received_ws = false;
    let mut received_quic = false;
    let mut sent_ws = false;
    let mut sent_quic = false;

    if let Some(adapter) = ws_receiver.as_mut() {
        if adapter.can_send() {
            let _ = adapter.send(&"server".to_string(), &ws_warmup);
        }
    }
    if ws_url.is_some() {
        thread::sleep(Duration::from_millis(200));
    }
    if let Some(adapter) = ws_sender.as_mut() {
        if adapter.can_send() {
            if adapter.send(&"server".to_string(), &ws_shard).is_ok() {
                println!("WS: sent test shard");
                sent_ws = true;
            }
        }
    }

    if let (Some(adapter), Some(peer)) = (quic_receiver.as_mut(), quic_peer.as_ref()) {
        let _ = adapter.send(peer, &quic_warmup);
    }
    if quic_peer.is_some() {
        thread::sleep(Duration::from_millis(200));
    }
    if let (Some(adapter), Some(peer)) = (quic_sender.as_mut(), quic_peer.as_ref()) {
        if adapter.send(peer, &quic_shard).is_ok() {
            println!("QUIC: sent test shard");
            sent_quic = true;
        }
    }

    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    while Instant::now() < deadline {
        if !received_ws {
            if let Some(adapter) = ws_receiver.as_mut() {
                if let Some((_peer, bytes)) = adapter.recv() {
                    if bytes == ws_shard {
                        println!("WS: received shard ({} bytes)", bytes.len());
                        received_ws = true;
                    }
                }
            }
        }
        if !received_quic {
            if let Some(adapter) = quic_receiver.as_mut() {
                if let Some((_peer, bytes)) = adapter.recv() {
                    if bytes == quic_shard {
                        println!("QUIC: received shard ({} bytes)", bytes.len());
                        received_quic = true;
                    }
                }
            }
        }
        if (ws_url.is_none() || received_ws) && (quic_peer.is_none() || received_quic) {
            break;
        }
        thread::sleep(Duration::from_millis(50));
    }

    println!("-- Summary");
    if ws_url.is_some() {
        println!("WS: sent={sent_ws} received={received_ws}");
    }
    if quic_peer.is_some() {
        println!("QUIC: sent={sent_quic} received={received_quic}");
    }

    if (ws_url.is_some() && (!sent_ws || !received_ws))
        || (quic_peer.is_some() && (!sent_quic || !received_quic))
    {
        eprintln!("Probe failed: one or more lanes did not round-trip.");
        std::process::exit(1);
    }
}

fn print_usage() {
    eprintln!(
        "Usage: cargo run -p veil-sim --bin vps_multilane_probe -- \
  --domain host \
  [--ws wss://host/ws] [--quic host:port] \\
  [--quic-cert-hex HEX|--quic-cert-b64 B64|--quic-cert-path PATH|--quic-cert-url https://host/veil/quic_cert.der] \
  [--tag HEX] [--namespace N] [--timeout SECONDS]"
    );
}

fn get_arg_value(args: &[String], key: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == key)
        .and_then(|idx| args.get(idx + 1))
        .map(|v| v.to_string())
}

fn parse_hex_tag(hex: &str) -> Result<Tag, String> {
    let cleaned = hex.trim().trim_start_matches("tag:").to_lowercase();
    if cleaned.len() != 64 {
        return Err("expected 64 hex chars".to_string());
    }
    let mut out = [0u8; 32];
    for i in 0..32 {
        let byte = u8::from_str_radix(&cleaned[i * 2..i * 2 + 2], 16)
            .map_err(|_| "invalid hex".to_string())?;
        out[i] = byte;
    }
    Ok(out)
}

fn build_ws_adapter(url: &str, peer_id: &str) -> WebSocketAdapter {
    WebSocketAdapter::connect(WebSocketAdapterConfig {
        url: url.to_string(),
        peer_id: peer_id.to_string(),
        reconnect: true,
        reconnect_initial: Duration::from_millis(250),
        reconnect_max: Duration::from_secs(10),
        outbound_queue_capacity: 1024,
        inbound_queue_capacity: 4096,
        max_payload_hint: Some(64 * 1024),
    })
    .expect("websocket adapter should start")
}

fn build_quic_adapter(
    peer: &str,
    cert_hex: Option<String>,
    cert_b64: Option<String>,
    cert_path: Option<String>,
    cert_url: Option<String>,
) -> Result<QuicAdapter, String> {
    let cert = if let Some(hex) = cert_hex {
        hex_to_bytes(&hex)?
    } else if let Some(b64) = cert_b64 {
        Base64Standard
            .decode(b64)
            .map_err(|_| "invalid base64 cert".to_string())?
    } else if let Some(path) = cert_path {
        std::fs::read(path).map_err(|err| format!("failed to read cert: {err}"))?
    } else if let Some(url) = cert_url {
        fetch_cert_from_url(&url)?
    } else {
        return Err(
            "QUIC cert required (--quic-cert-hex/b64/path or --quic-cert-url)".to_string(),
        );
    };

    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0);
    let identity = QuicIdentity::generate_self_signed("veil-probe")
        .map_err(|_| "failed to generate QUIC identity".to_string())?;
    let cfg = QuicAdapterConfig {
        bind_addr,
        server_name: "veil-node".to_string(),
        identity,
        trusted_peer_certs_der: vec![cert],
        connect_timeout: Duration::from_secs(3),
        send_timeout: Duration::from_secs(3),
        outbound_queue_capacity: 1024,
        inbound_queue_capacity: 4096,
        max_recv_bytes: 128 * 1024,
        max_payload_hint: Some(64 * 1024),
    };
    let adapter = QuicAdapter::connect(cfg).map_err(|err| format!("{err}"))?;
    // Send a dummy packet to open connection (optional).
    let _ = adapter.can_send();
    let _ = peer;
    Ok(adapter)
}

fn wait_for_ws_ready(sender: Option<&mut WebSocketAdapter>, receiver: Option<&mut WebSocketAdapter>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        let sender_ready = sender.as_ref().map(|a| a.can_send()).unwrap_or(true);
        let receiver_ready = receiver.as_ref().map(|a| a.can_send()).unwrap_or(true);
        if sender_ready && receiver_ready {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn build_shard(tag: Tag, namespace: Namespace, epoch: Epoch, object_root: [u8; 32], index: u16) -> Vec<u8> {
    let payload_len = 16 * 1024 - SHARD_HEADER_LEN;
    let payload = vec![0x42_u8; payload_len];
    let shard = ShardV1 {
        header: ShardHeaderV1 {
            version: 1,
            namespace,
            epoch,
            tag,
            object_root,
            k: 2,
            n: 3,
            index,
        },
        payload,
    };
    encode_shard_cbor(&shard).expect("shard should encode")
}

fn current_epoch() -> Epoch {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    Epoch((now.as_secs() / 86_400) as u32)
}

fn hex_to_bytes(hex: &str) -> Result<Vec<u8>, String> {
    let cleaned = hex.trim().trim_start_matches("0x").replace(':', "");
    if cleaned.len() % 2 != 0 {
        return Err("invalid hex length".to_string());
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let mut i = 0;
    while i < cleaned.len() {
        let byte = u8::from_str_radix(&cleaned[i..i + 2], 16)
            .map_err(|_| "invalid hex".to_string())?;
        out.push(byte);
        i += 2;
    }
    Ok(out)
}

fn fetch_cert_from_url(url: &str) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err("QUIC cert URL must be https://".to_string());
    }
    let agent = ureq::AgentBuilder::new()
        .timeout_connect(Duration::from_secs(5))
        .timeout_read(Duration::from_secs(8))
        .build();
    let response = agent.get(url).call()
        .map_err(|err| format!("failed to fetch cert: {err}"))?;
    if response.status() >= 400 {
        return Err(format!("cert fetch failed: HTTP {}", response.status()));
    }
    let mut bytes = Vec::new();
    response
        .into_reader()
        .read_to_end(&mut bytes)
        .map_err(|err| format!("failed to read cert: {err}"))?;
    if bytes.is_empty() {
        return Err("cert fetch returned empty body".to_string());
    }
    Ok(bytes)
}
