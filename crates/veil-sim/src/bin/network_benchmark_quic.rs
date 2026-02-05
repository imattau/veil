use std::collections::HashMap;
use std::env;
use std::fs;
use std::net::{SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::Serialize;
use veil_transport::adapter::TransportAdapter;
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};

const MSG_DATA: u8 = 1;
const MSG_ACK: u8 = 2;

#[derive(Debug, Clone, Serialize)]
struct QuicBenchResult {
    generated_at_unix_seconds: u64,
    count: usize,
    payload_bytes: usize,
    timeout_ms: u64,
    sent_count: usize,
    receiver_data_count: usize,
    acked_count: usize,
    elapsed_ms: u128,
    throughput_mib_s: f64,
    latency_p50_ms: f64,
    latency_p95_ms: f64,
    sender_health: HealthSnapshotView,
    receiver_health: HealthSnapshotView,
}

#[derive(Debug, Clone, Serialize)]
struct HealthSnapshotView {
    outbound_queued: u64,
    outbound_send_ok: u64,
    outbound_send_err: u64,
    inbound_received: u64,
    inbound_dropped: u64,
    reconnect_attempts: u64,
}

impl From<veil_transport::adapter::TransportHealthSnapshot> for HealthSnapshotView {
    fn from(value: veil_transport::adapter::TransportHealthSnapshot) -> Self {
        Self {
            outbound_queued: value.outbound_queued,
            outbound_send_ok: value.outbound_send_ok,
            outbound_send_err: value.outbound_send_err,
            inbound_received: value.inbound_received,
            inbound_dropped: value.inbound_dropped,
            reconnect_attempts: value.reconnect_attempts,
        }
    }
}

fn free_udp_addr() -> std::io::Result<SocketAddr> {
    let sock = UdpSocket::bind("127.0.0.1:0")?;
    sock.local_addr()
}

fn parse_arg_usize(args: &[String], key: &str, default: usize) -> usize {
    args.windows(2)
        .find(|w| w[0] == key)
        .and_then(|w| w[1].parse::<usize>().ok())
        .unwrap_or(default)
}

fn parse_arg_u64(args: &[String], key: &str, default: u64) -> u64 {
    args.windows(2)
        .find(|w| w[0] == key)
        .and_then(|w| w[1].parse::<u64>().ok())
        .unwrap_or(default)
}

fn parse_arg_path(args: &[String], key: &str, default: &str) -> PathBuf {
    args.windows(2)
        .find(|w| w[0] == key)
        .map(|w| PathBuf::from(&w[1]))
        .unwrap_or_else(|| PathBuf::from(default))
}

fn percentile_ms(values: &[f64], percentile: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() as f64) * percentile).ceil() as usize - 1;
    sorted[idx.min(sorted.len() - 1)]
}

fn make_data_packet(seq: u64, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 8 + payload.len());
    out.push(MSG_DATA);
    out.extend_from_slice(&seq.to_be_bytes());
    out.extend_from_slice(payload);
    out
}

fn make_ack_packet(seq: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(1 + 8);
    out.push(MSG_ACK);
    out.extend_from_slice(&seq.to_be_bytes());
    out
}

fn parse_seq(bytes: &[u8], expected_type: u8) -> Option<u64> {
    if bytes.len() < 9 || bytes[0] != expected_type {
        return None;
    }
    let mut seq = [0_u8; 8];
    seq.copy_from_slice(&bytes[1..9]);
    Some(u64::from_be_bytes(seq))
}

fn write_outputs(out_dir: &Path, result: &QuicBenchResult) -> std::io::Result<()> {
    fs::create_dir_all(out_dir)?;
    let json_path = out_dir.join("quic_network_report.json");
    let csv_path = out_dir.join("quic_network_report.csv");

    let json = serde_json::to_string_pretty(result).expect("json serialize should work");
    fs::write(json_path, json)?;

    let mut csv = String::from(
        "count,payload_bytes,timeout_ms,sent_count,receiver_data_count,acked_count,elapsed_ms,throughput_mib_s,latency_p50_ms,latency_p95_ms,sender_outbound_ok,sender_outbound_err,receiver_inbound_received\n",
    );
    csv.push_str(&format!(
        "{},{},{},{},{},{},{},{:.6},{:.3},{:.3},{},{},{}\n",
        result.count,
        result.payload_bytes,
        result.timeout_ms,
        result.sent_count,
        result.receiver_data_count,
        result.acked_count,
        result.elapsed_ms,
        result.throughput_mib_s,
        result.latency_p50_ms,
        result.latency_p95_ms,
        result.sender_health.outbound_send_ok,
        result.sender_health.outbound_send_err,
        result.receiver_health.inbound_received
    ));
    fs::write(csv_path, csv)?;
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--help") {
        println!(
            "Usage: cargo run -p veil-sim --bin network_benchmark_quic -- [--count N] [--payload-bytes N] [--timeout-ms N] [--out-dir DIR]"
        );
        return Ok(());
    }

    let count = parse_arg_usize(&args, "--count", 512);
    let payload_bytes = parse_arg_usize(&args, "--payload-bytes", 16 * 1024);
    let timeout_ms = parse_arg_u64(&args, "--timeout-ms", 30_000);
    let out_dir = parse_arg_path(&args, "--out-dir", "target/benchmarks/veil-sim");

    let identity_sender =
        QuicIdentity::generate_self_signed("localhost").map_err(|e| e.to_string())?;
    let identity_receiver =
        QuicIdentity::generate_self_signed("localhost").map_err(|e| e.to_string())?;

    let sender_addr =
        free_udp_addr().map_err(|e| format!("failed to allocate sender UDP port: {e}"))?;
    let receiver_addr =
        free_udp_addr().map_err(|e| format!("failed to allocate receiver UDP port: {e}"))?;

    let mut sender_cfg = QuicAdapterConfig::new(sender_addr, "localhost", identity_sender.clone());
    sender_cfg.trusted_peer_certs_der = vec![identity_receiver.cert_der.clone()];
    let mut receiver_cfg =
        QuicAdapterConfig::new(receiver_addr, "localhost", identity_receiver.clone());
    receiver_cfg.trusted_peer_certs_der = vec![identity_sender.cert_der.clone()];

    let mut sender =
        QuicAdapter::connect(sender_cfg).map_err(|e| format!("sender setup failed: {e}"))?;
    let mut receiver =
        QuicAdapter::connect(receiver_cfg).map_err(|e| format!("receiver setup failed: {e}"))?;

    let receiver_peer = receiver_addr.to_string();
    let payload = vec![0xAB_u8; payload_bytes];
    let mut sent_at = HashMap::<u64, Instant>::new();
    let mut ack_latencies_ms = Vec::<f64>::new();
    let mut sent_count = 0usize;
    let mut receiver_data_count = 0usize;
    let mut acked_count = 0usize;

    let start = Instant::now();

    for seq in 0..count as u64 {
        let packet = make_data_packet(seq, &payload);
        loop {
            match sender.send(&receiver_peer, &packet) {
                Ok(()) => {
                    sent_count += 1;
                    sent_at.insert(seq, Instant::now());
                    break;
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(1));
                }
            }
        }
    }

    let timeout = Duration::from_millis(timeout_ms);
    while acked_count < count && start.elapsed() < timeout {
        while let Some((from_peer, bytes)) = receiver.recv() {
            if let Some(seq) = parse_seq(&bytes, MSG_DATA) {
                receiver_data_count += 1;
                let ack = make_ack_packet(seq);
                let _ = receiver.send(&from_peer, &ack);
            }
        }

        while let Some((_from_peer, bytes)) = sender.recv() {
            if let Some(seq) = parse_seq(&bytes, MSG_ACK) {
                if let Some(t0) = sent_at.remove(&seq) {
                    acked_count += 1;
                    ack_latencies_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
                }
            }
        }
        thread::sleep(Duration::from_millis(1));
    }

    let elapsed = start.elapsed();
    let throughput_mib_s = ((acked_count * payload_bytes) as f64 / (1024.0 * 1024.0))
        / elapsed.as_secs_f64().max(1e-9);
    let result = QuicBenchResult {
        generated_at_unix_seconds: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        count,
        payload_bytes,
        timeout_ms,
        sent_count,
        receiver_data_count,
        acked_count,
        elapsed_ms: elapsed.as_millis(),
        throughput_mib_s,
        latency_p50_ms: percentile_ms(&ack_latencies_ms, 0.50),
        latency_p95_ms: percentile_ms(&ack_latencies_ms, 0.95),
        sender_health: sender.health_snapshot().into(),
        receiver_health: receiver.health_snapshot().into(),
    };

    write_outputs(&out_dir, &result).map_err(|e| format!("write outputs failed: {e}"))?;
    println!("Wrote QUIC network benchmark report:");
    println!("  {}", out_dir.join("quic_network_report.json").display());
    println!("  {}", out_dir.join("quic_network_report.csv").display());
    println!(
        "acked={}/{} throughput={:.2} MiB/s p50={:.2}ms p95={:.2}ms",
        result.acked_count,
        result.count,
        result.throughput_mib_s,
        result.latency_p50_ms,
        result.latency_p95_ms
    );
    Ok(())
}

fn main() {
    if let Err(err) = run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
