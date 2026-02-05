use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use rand::seq::SliceRandom;
use rand::{rngs::StdRng, Rng, RngCore, SeedableRng};
use serde::Serialize;
use veil_codec::object::{
    encode_object_cbor, object_signature_message_digest, ObjectV1, Signature, OBJECT_FLAG_SIGNED,
    OBJECT_V1_VERSION,
};
use veil_codec::shard::encode_shard_cbor;
use veil_core::hash::blake3_32;
use veil_core::{Epoch, Namespace, ObjectRoot};
use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
use veil_crypto::signing::{Ed25519Signer, Ed25519Verifier, Signer};
use veil_fec::sharder::{derive_object_root, object_to_shards};
use veil_node::receive::ReceiveEvent;
use veil_node::runtime::{pump_once, PumpParams, RuntimePolicyHooks, RuntimeStats};
use veil_node::state::NodeState;
use veil_transport::adapter::InMemoryAdapter;

#[derive(Debug, Clone, Copy)]
struct BenchCase {
    name: &'static str,
    objects: usize,
    payload_min: usize,
    payload_max: usize,
    noisy_mix: bool,
}

#[derive(Debug, Clone, Serialize)]
struct BenchResult {
    case: String,
    seed: u64,
    objects: usize,
    delivered_objects: usize,
    delivered_bytes: usize,
    elapsed_ms: u128,
    throughput_mib_s: f64,
    latency_p50_steps: u64,
    latency_p95_steps: u64,
    inbound_messages: usize,
    parsed_shards: usize,
    malformed_messages: usize,
    duplicate_messages: usize,
    ignored_messages: usize,
    send_failures: usize,
}

#[derive(Debug, Clone, Serialize)]
struct BenchReport {
    generated_at_unix_seconds: u64,
    results: Vec<BenchResult>,
}

#[derive(Debug, Clone)]
struct ExpectedDelivery {
    payload: Vec<u8>,
    enqueued_step: u64,
}

fn pxx(values: &[u64], percentile: f64) -> u64 {
    if values.is_empty() {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let idx = ((sorted.len() as f64) * percentile).ceil() as usize - 1;
    sorted[idx.min(sorted.len() - 1)]
}

fn build_signed_encrypted_object(
    payload: &[u8],
    namespace: Namespace,
    epoch: Epoch,
    tag: [u8; 32],
    key: &[u8; 32],
    nonce: [u8; 24],
    signer: &Ed25519Signer,
) -> Vec<u8> {
    let cipher = XChaCha20Poly1305Cipher;
    let aad = build_veil_aad(tag, namespace, epoch);
    let envelope = cipher
        .encrypt(key, nonce, &aad, payload)
        .expect("encryption should succeed");

    let mut object = ObjectV1 {
        version: OBJECT_V1_VERSION,
        namespace,
        epoch,
        flags: OBJECT_FLAG_SIGNED,
        tag,
        object_root: derive_object_root(payload),
        sender_pubkey: Some(signer.public_key()),
        signature: Some(Signature([0_u8; 64])),
        nonce: envelope.nonce,
        ciphertext: envelope.ciphertext,
        padding: vec![0_u8; 16],
    };
    let digest = object_signature_message_digest(&object).expect("digest should compute");
    object.signature = Some(Signature(
        signer.sign(&digest).expect("signature should succeed"),
    ));
    encode_object_cbor(&object).expect("object should encode")
}

fn run_case(seed: u64, case: BenchCase) -> BenchResult {
    let mut rng = StdRng::seed_from_u64(seed);
    let signer = Ed25519Signer::from_secret([0x77_u8; 32]);
    let key = [0xA5_u8; 32];
    let tag = [0x44_u8; 32];

    let mut node = NodeState::default();
    node.subscriptions.insert(tag);
    let mut adapter = InMemoryAdapter::default();
    let mut stats = RuntimeStats::default();
    let mut expected = HashMap::<ObjectRoot, ExpectedDelivery>::new();
    let mut delivered_count = 0usize;
    let mut delivered_bytes = 0usize;
    let mut latencies_steps = Vec::<u64>::new();
    let mut step = 0u64;
    let peers = vec![
        "sender".to_string(),
        "peer-a".to_string(),
        "peer-b".to_string(),
    ];

    let start = Instant::now();
    for idx in 0..case.objects {
        let payload_len = rng.gen_range(case.payload_min..=case.payload_max);
        let mut payload = vec![0_u8; payload_len];
        rng.fill_bytes(&mut payload);

        let namespace = Namespace((idx % u16::MAX as usize) as u16);
        let epoch = Epoch(20_000 + idx as u32);
        let mut nonce = [0_u8; 24];
        rng.fill_bytes(&mut nonce);

        let encoded =
            build_signed_encrypted_object(&payload, namespace, epoch, tag, &key, nonce, &signer);
        let wire_root = blake3_32(&encoded);
        expected.insert(
            wire_root,
            ExpectedDelivery {
                payload,
                enqueued_step: step,
            },
        );

        let mut shards = object_to_shards(&encoded, namespace, epoch, tag, wire_root)
            .expect("sharding should succeed");
        let k = shards[0].header.k as usize;
        let n = shards[0].header.n as usize;
        shards.shuffle(&mut rng);

        let recv_count = if case.noisy_mix {
            rng.gen_range(k..=n)
        } else {
            k
        };
        for shard in shards.iter().take(recv_count) {
            let bytes = encode_shard_cbor(shard).expect("shard encode should succeed");
            adapter.enqueue_inbound("sender", bytes.clone());
            if case.noisy_mix && rng.gen_bool(0.12) {
                adapter.enqueue_inbound("sender", bytes);
            }
        }
        if case.noisy_mix && idx % 7 == 0 {
            adapter.enqueue_inbound("sender", vec![0xDE, 0xAD, 0xBE, 0xEF]);
        }

        loop {
            let event = pump_once(
                &mut node,
                &mut adapter,
                PumpParams {
                    peers: &peers,
                    now_step: step,
                    ttl_steps: 10_000,
                    fanout: 1,
                    policy_hooks: RuntimePolicyHooks::default(),
                    decrypt_key: &key,
                    stats: &mut stats,
                },
                &XChaCha20Poly1305Cipher,
                &Ed25519Verifier,
            )
            .expect("pump should run");
            step += 1;
            let Some(event) = event else {
                break;
            };
            if let ReceiveEvent::Delivered {
                object_root,
                payload,
                ..
            } = event
            {
                let exp = expected
                    .remove(&object_root)
                    .expect("delivered root should be expected");
                assert_eq!(payload, exp.payload);
                latencies_steps.push(step.saturating_sub(exp.enqueued_step));
                delivered_count += 1;
                delivered_bytes += payload.len();
            }
        }
        let _ = adapter.take_outbound();
    }

    let elapsed = start.elapsed();
    let secs = elapsed.as_secs_f64().max(1e-9);
    BenchResult {
        case: case.name.to_string(),
        seed,
        objects: case.objects,
        delivered_objects: delivered_count,
        delivered_bytes,
        elapsed_ms: elapsed.as_millis(),
        throughput_mib_s: (delivered_bytes as f64 / (1024.0 * 1024.0)) / secs,
        latency_p50_steps: pxx(&latencies_steps, 0.50),
        latency_p95_steps: pxx(&latencies_steps, 0.95),
        inbound_messages: stats.inbound_messages,
        parsed_shards: stats.parsed_shards,
        malformed_messages: stats.malformed_messages,
        duplicate_messages: stats.duplicate_messages,
        ignored_messages: stats.ignored_messages,
        send_failures: stats.send_failures,
    }
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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|a| a == flag)
}

fn write_outputs(output_dir: &Path, report: &BenchReport) -> std::io::Result<()> {
    fs::create_dir_all(output_dir)?;
    let json_path = output_dir.join("bench_report.json");
    let csv_path = output_dir.join("bench_report.csv");

    let json = serde_json::to_string_pretty(report).expect("json serialize should work");
    fs::write(json_path, json)?;

    let mut csv = String::from(
        "case,seed,objects,delivered_objects,delivered_bytes,elapsed_ms,throughput_mib_s,latency_p50_steps,latency_p95_steps,inbound_messages,parsed_shards,malformed_messages,duplicate_messages,ignored_messages,send_failures\n",
    );
    for row in &report.results {
        let line = format!(
            "{},{},{},{},{},{},{:.6},{},{},{},{},{},{},{},{}\n",
            row.case,
            row.seed,
            row.objects,
            row.delivered_objects,
            row.delivered_bytes,
            row.elapsed_ms,
            row.throughput_mib_s,
            row.latency_p50_steps,
            row.latency_p95_steps,
            row.inbound_messages,
            row.parsed_shards,
            row.malformed_messages,
            row.duplicate_messages,
            row.ignored_messages,
            row.send_failures
        );
        csv.push_str(&line);
    }
    fs::write(csv_path, csv)?;
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if has_flag(&args, "--help") {
        println!(
            "Usage: cargo run -p veil-sim --bin benchmark_runner -- [--seed N] [--out-dir DIR] [--quick]\n\
             --quick runs smaller object counts for fast local checks."
        );
        return;
    }

    let seed = parse_arg_u64(&args, "--seed", 0xBEEF_CAFE);
    let out_dir = parse_arg_path(&args, "--out-dir", "target/benchmarks/veil-sim");
    let quick = has_flag(&args, "--quick");

    let smoke = BenchCase {
        name: "smoke",
        objects: if quick { 16 } else { 50 },
        payload_min: 8 * 1024,
        payload_max: 96 * 1024,
        noisy_mix: false,
    };
    let load = BenchCase {
        name: "load_noise",
        objects: if quick { 36 } else { 120 },
        payload_min: 2 * 1024,
        payload_max: 210 * 1024,
        noisy_mix: true,
    };

    let report = BenchReport {
        generated_at_unix_seconds: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        results: vec![run_case(seed, smoke), run_case(seed ^ 0xA11CE, load)],
    };

    write_outputs(&out_dir, &report).expect("writing benchmark outputs should succeed");

    println!("Wrote benchmark report:");
    println!("  {}", out_dir.join("bench_report.json").display());
    println!("  {}", out_dir.join("bench_report.csv").display());
    for row in &report.results {
        println!(
            "- {}: {:.2} MiB/s, p95={} steps, delivered={}/{}",
            row.case,
            row.throughput_mib_s,
            row.latency_p95_steps,
            row.delivered_objects,
            row.objects
        );
    }
}
