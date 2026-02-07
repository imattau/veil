#![allow(unexpected_cfgs)]

mod frb_generated; /* AUTO INJECTED BY flutter_rust_bridge. This line may not be accurate, and you can change it according to your needs. */
#[allow(unused_imports)]
use flutter_rust_bridge::frb;

#[frb]
pub mod api {
    use veil_codec::object::{
        decode_object_cbor, OBJECT_FLAG_ACK_REQUESTED, OBJECT_FLAG_BATCHED, OBJECT_FLAG_PUBLIC,
        OBJECT_FLAG_SIGNED,
    };
    use veil_codec::shard::decode_shard_cbor;
    use veil_core::hash::blake3_32;
    use veil_core::tags::{current_epoch, derive_feed_tag, derive_rv_tag};
    use veil_core::types::{Epoch, Namespace};
    use veil_core::{ObjectRoot, Tag};
    use veil_crypto::aead::{build_veil_aad, AeadCipher, XChaCha20Poly1305Cipher};
    use veil_fec::sharder::reconstruct_object_padded;

    #[derive(Clone, Debug)]
    #[frb]
    pub struct ShardMeta {
        pub version: u16,
        pub namespace: u16,
        pub epoch: u32,
        pub tag_hex: String,
        pub object_root_hex: String,
        pub k: u16,
        pub n: u16,
        pub index: u16,
        pub payload_len: usize,
    }

    #[derive(Clone, Debug)]
    #[frb]
    pub struct ObjectMeta {
        pub version: u16,
        pub namespace: u16,
        pub epoch: u32,
        pub flags: u16,
        pub signed: bool,
        pub public: bool,
        pub ack_requested: bool,
        pub batched: bool,
        pub tag_hex: String,
        pub object_root_hex: String,
        pub sender_pubkey_hex: Option<String>,
        pub nonce_hex: String,
        pub ciphertext_len: usize,
        pub padding_len: usize,
    }

    fn hex_encode(bytes: &[u8]) -> String {
        let mut out = String::with_capacity(bytes.len() * 2);
        for b in bytes {
            out.push_str(&format!("{:02x}", b));
        }
        out
    }

    fn hex_decode_32(input: &str) -> Result<[u8; 32], String> {
        if input.len() != 64 {
            return Err("expected 64 hex chars".to_string());
        }
        let mut out = [0_u8; 32];
        for (i, chunk) in input.as_bytes().chunks_exact(2).enumerate() {
            let s = std::str::from_utf8(chunk).map_err(|_| "invalid hex")?;
            out[i] = u8::from_str_radix(s, 16).map_err(|_| "invalid hex")?;
        }
        Ok(out)
    }

    #[frb]
    pub fn derive_feed_tag_hex(
        publisher_pubkey_hex: String,
        namespace: u16,
    ) -> Result<String, String> {
        let key = hex_decode_32(&publisher_pubkey_hex)?;
        let tag: Tag = derive_feed_tag(&key, Namespace(namespace));
        Ok(hex_encode(&tag))
    }

    #[frb]
    pub fn derive_rv_tag_hex(
        recipient_pubkey_hex: String,
        epoch: u32,
        namespace: u16,
    ) -> Result<String, String> {
        let key = hex_decode_32(&recipient_pubkey_hex)?;
        let tag: Tag = derive_rv_tag(&key, Epoch(epoch), Namespace(namespace));
        Ok(hex_encode(&tag))
    }

    #[frb]
    pub fn current_epoch_seconds(now: u64, epoch_seconds: u64) -> u64 {
        current_epoch(now, epoch_seconds).0 as u64
    }

    #[frb]
    pub fn decode_shard_meta(bytes: Vec<u8>) -> Result<ShardMeta, String> {
        let shard = decode_shard_cbor(&bytes).map_err(|e| e.to_string())?;
        Ok(ShardMeta {
            version: shard.header.version,
            namespace: shard.header.namespace.0,
            epoch: shard.header.epoch.0,
            tag_hex: hex_encode(&shard.header.tag),
            object_root_hex: hex_encode(&shard.header.object_root),
            k: shard.header.k,
            n: shard.header.n,
            index: shard.header.index,
            payload_len: shard.payload.len(),
        })
    }

    #[frb]
    pub fn decode_object_meta(bytes: Vec<u8>) -> Result<ObjectMeta, String> {
        let obj = decode_object_cbor(&bytes).map_err(|e| e.to_string())?;
        let flags = obj.flags;
        Ok(ObjectMeta {
            version: obj.version,
            namespace: obj.namespace.0,
            epoch: obj.epoch.0,
            flags,
            signed: flags & OBJECT_FLAG_SIGNED != 0,
            public: flags & OBJECT_FLAG_PUBLIC != 0,
            ack_requested: flags & OBJECT_FLAG_ACK_REQUESTED != 0,
            batched: flags & OBJECT_FLAG_BATCHED != 0,
            tag_hex: hex_encode(&obj.tag),
            object_root_hex: hex_encode(&obj.object_root),
            sender_pubkey_hex: obj.sender_pubkey.map(|p| hex_encode(&p)),
            nonce_hex: hex_encode(&obj.nonce),
            ciphertext_len: obj.ciphertext.len(),
            padding_len: obj.padding.len(),
        })
    }

    #[frb]
    pub fn derive_object_root_hex(object_bytes: Vec<u8>) -> String {
        let root: ObjectRoot = blake3_32(&object_bytes);
        hex_encode(&root)
    }

    #[frb]
    pub fn reconstruct_object_padded_from_shards(
        shard_bytes: Vec<Vec<u8>>,
        expected_root_hex: String,
    ) -> Result<Vec<u8>, String> {
        let expected_root = hex_decode_32(&expected_root_hex)?;
        let mut shards = Vec::with_capacity(shard_bytes.len());
        for bytes in shard_bytes {
            let shard = decode_shard_cbor(&bytes).map_err(|e| e.to_string())?;
            shards.push(shard);
        }
        reconstruct_object_padded(&shards, expected_root).map_err(|e| e.to_string())
    }

    #[frb]
    pub fn decrypt_object_payload(
        object_bytes: Vec<u8>,
        key_bytes: Vec<u8>,
    ) -> Result<Vec<u8>, String> {
        if key_bytes.len() != 32 {
            return Err("decrypt key must be 32 bytes".to_string());
        }
        let obj = decode_object_cbor(&object_bytes).map_err(|e| e.to_string())?;
        let aad = build_veil_aad(obj.tag, obj.namespace, obj.epoch);
        let cipher = XChaCha20Poly1305Cipher;
        cipher
            .decrypt(&key_bytes, obj.nonce, &aad, &obj.ciphertext)
            .map_err(|e| e.to_string())
    }
}

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::net::SocketAddr;
use std::os::raw::{c_char, c_int};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use veil_transport::adapter::TransportAdapter;
use veil_transport_quic::{QuicAdapter, QuicAdapterConfig, QuicIdentity};

static QUIC_HANDLES: Lazy<Mutex<HashMap<u64, QuicAdapter>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static QUIC_NEXT_ID: AtomicU64 = AtomicU64::new(1);

#[repr(C)]
pub struct QuicRecv {
    pub peer: *mut c_char,
    pub data: *mut u8,
    pub len: usize,
}

fn decode_hex_vec(input: &str) -> Result<Vec<u8>, String> {
    let bytes = input.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return Err("expected even-length hex string".to_string());
    }
    let mut out = Vec::with_capacity(bytes.len() / 2);
    for chunk in bytes.chunks_exact(2) {
        let s = std::str::from_utf8(chunk).map_err(|_| "invalid hex")?;
        out.push(u8::from_str_radix(s, 16).map_err(|_| "invalid hex")?);
    }
    Ok(out)
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null string pointer".to_string());
    }
    let cstr = unsafe { CStr::from_ptr(ptr) };
    cstr.to_str()
        .map(|s| s.to_string())
        .map_err(|_| "invalid utf8".to_string())
}

fn hex_encode_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn strip_scheme(input: &str) -> &str {
    if let Some(idx) = input.find("://") {
        &input[idx + 3..]
    } else {
        input
    }
}

#[no_mangle]
pub extern "C" fn veil_quic_is_supported() -> c_int {
    1
}

#[no_mangle]
pub extern "C" fn veil_quic_start(
    bind_addr: *const c_char,
    server_name: *const c_char,
    trusted_peer_cert_hex: *const c_char,
) -> u64 {
    let bind_addr = match cstr_to_string(bind_addr)
        .ok()
        .and_then(|s| strip_scheme(&s).parse::<SocketAddr>().ok())
    {
        Some(addr) => addr,
        None => return 0,
    };
    let server_name = match cstr_to_string(server_name) {
        Ok(value) if !value.is_empty() => value,
        _ => return 0,
    };

    let identity = match QuicIdentity::generate_self_signed(&server_name) {
        Ok(id) => id,
        Err(_) => return 0,
    };

    let mut config = QuicAdapterConfig::new(bind_addr, &server_name, identity.clone());
    if !trusted_peer_cert_hex.is_null() {
        if let Ok(hex) = cstr_to_string(trusted_peer_cert_hex) {
            if !hex.is_empty() {
                if let Ok(bytes) = decode_hex_vec(&hex) {
                    config.trusted_peer_certs_der = vec![bytes];
                }
            }
        }
    }

    let adapter = match QuicAdapter::connect(config) {
        Ok(adapter) => adapter,
        Err(_) => return 0,
    };

    let id = QUIC_NEXT_ID.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut map) = QUIC_HANDLES.lock() {
        map.insert(id, adapter);
    }
    id
}

#[no_mangle]
pub extern "C" fn veil_quic_fetch_peer_cert(
    endpoint: *const c_char,
    server_name: *const c_char,
) -> *mut c_char {
    let endpoint = match cstr_to_string(endpoint) {
        Ok(value) => value,
        Err(_) => return std::ptr::null_mut(),
    };
    let server_name = match cstr_to_string(server_name) {
        Ok(value) if !value.is_empty() => value,
        _ => return std::ptr::null_mut(),
    };
    let addr = match strip_scheme(&endpoint).parse::<SocketAddr>() {
        Ok(addr) => addr,
        Err(_) => return std::ptr::null_mut(),
    };

    let cert_store: Arc<Mutex<Option<Vec<u8>>>> = Arc::new(Mutex::new(None));
    let cert_store_clone = Arc::clone(&cert_store);

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(_) => return std::ptr::null_mut(),
    };

    let result = runtime.block_on(async move {
        let verifier = RecordingVerifier {
            store: cert_store_clone,
        };
        let client_cfg = build_insecure_client_config(verifier)?;
        let mut endpoint = quinn::Endpoint::client("0.0.0.0:0".parse().unwrap())
            .map_err(|_| "endpoint init failed")?;
        endpoint.set_default_client_config(client_cfg);
        let connecting = endpoint
            .connect(addr, &server_name)
            .map_err(|_| "connect failed")?;
        let _connection = tokio::time::timeout(Duration::from_secs(3), connecting)
            .await
            .map_err(|_| "connect timeout")?
            .map_err(|_| "connect error")?;
        endpoint.wait_idle().await;
        Ok::<(), String>(())
    });

    if result.is_err() {
        return std::ptr::null_mut();
    }

    let cert = cert_store.lock().ok().and_then(|guard| guard.clone());
    let Some(cert) = cert else {
        return std::ptr::null_mut();
    };
    let hex = hex_encode_bytes(&cert);
    match CString::new(hex) {
        Ok(cstr) => cstr.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

/// # Safety
/// `ptr` must be a pointer returned by `veil_quic_fetch_peer_cert` and must be
/// freed exactly once.
#[no_mangle]
pub unsafe extern "C" fn veil_quic_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    let _ = CString::from_raw(ptr);
}

/// # Safety
/// `peer` must be a valid NUL-terminated C string and `data` must point to a
/// buffer of at least `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn veil_quic_send(
    handle: u64,
    peer: *const c_char,
    data: *const u8,
    len: usize,
) -> c_int {
    if handle == 0 || data.is_null() || len == 0 {
        return -1;
    }
    let peer = match cstr_to_string(peer) {
        Ok(p) => p,
        Err(_) => return -2,
    };
    let bytes = std::slice::from_raw_parts(data, len);
    let mut map = match QUIC_HANDLES.lock() {
        Ok(map) => map,
        Err(_) => return -3,
    };
    let adapter = match map.get_mut(&handle) {
        Some(adapter) => adapter,
        None => return -4,
    };
    match adapter.send(&peer, bytes) {
        Ok(_) => 0,
        Err(_) => -5,
    }
}

#[no_mangle]
pub extern "C" fn veil_quic_recv(handle: u64) -> *mut QuicRecv {
    if handle == 0 {
        return std::ptr::null_mut();
    }
    let mut map = match QUIC_HANDLES.lock() {
        Ok(map) => map,
        Err(_) => return std::ptr::null_mut(),
    };
    let adapter = match map.get_mut(&handle) {
        Some(adapter) => adapter,
        None => return std::ptr::null_mut(),
    };
    let Some((peer, bytes)) = adapter.recv() else {
        return std::ptr::null_mut();
    };
    let peer_c = match CString::new(peer) {
        Ok(cstr) => cstr,
        Err(_) => return std::ptr::null_mut(),
    };
    let mut data: Vec<u8> = bytes;
    let len = data.len();
    let data_ptr = data.as_mut_ptr();
    std::mem::forget(data);
    let recv = QuicRecv {
        peer: peer_c.into_raw(),
        data: data_ptr,
        len,
    };
    Box::into_raw(Box::new(recv))
}

/// # Safety
/// `ptr` must be a pointer returned by `veil_quic_recv` and must be freed
/// exactly once.
#[no_mangle]
pub unsafe extern "C" fn veil_quic_free_recv(ptr: *mut QuicRecv) {
    if ptr.is_null() {
        return;
    }
    let recv = Box::from_raw(ptr);
    if !recv.peer.is_null() {
        let _ = CString::from_raw(recv.peer);
    }
    if !recv.data.is_null() && recv.len > 0 {
        let _ = Vec::from_raw_parts(recv.data, recv.len, recv.len);
    }
}

#[no_mangle]
pub extern "C" fn veil_quic_stop(handle: u64) {
    if handle == 0 {
        return;
    }
    if let Ok(mut map) = QUIC_HANDLES.lock() {
        map.remove(&handle);
    }
}

#[derive(Debug)]
struct RecordingVerifier {
    store: Arc<Mutex<Option<Vec<u8>>>>,
}

impl RecordingVerifier {
    fn record(&self, cert: &[u8]) {
        if let Ok(mut guard) = self.store.lock() {
            *guard = Some(cert.to_vec());
        }
    }
}

impl rustls::client::danger::ServerCertVerifier for RecordingVerifier {
    fn verify_server_cert(
        &self,
        end_entity: &rustls_pki_types::CertificateDer<'_>,
        _intermediates: &[rustls_pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        self.record(end_entity.as_ref());
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls_pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ED25519,
        ]
    }
}

fn build_insecure_client_config(
    verifier: RecordingVerifier,
) -> Result<quinn::ClientConfig, String> {
    let tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    let quic_tls =
        quinn::crypto::rustls::QuicClientConfig::try_from(tls).map_err(|_| "invalid tls config")?;
    Ok(quinn::ClientConfig::new(Arc::new(quic_tls)))
}
