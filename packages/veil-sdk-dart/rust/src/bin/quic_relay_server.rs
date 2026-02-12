use std::io::{self, Write};
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use quinn::{ClientConfig, Endpoint, ServerConfig};
use rustls_pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use veil_transport_quic::QuicIdentity;

fn free_udp_addr() -> SocketAddr {
    let sock = UdpSocket::bind("127.0.0.1:0").expect("bind should work");
    sock.local_addr().expect("local addr should resolve")
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        out.push_str(&format!("{:02x}", b));
    }
    out
}

fn build_server_config(identity: &QuicIdentity) -> Result<ServerConfig, String> {
    let cert = CertificateDer::from(identity.cert_der.clone());
    let key = PrivateKeyDer::from(PrivatePkcs8KeyDer::from(identity.key_der.clone()));
    ServerConfig::with_single_cert(vec![cert], key).map_err(|_| "invalid identity".to_string())
}

fn build_insecure_client_config() -> Result<ClientConfig, String> {
    #[derive(Debug)]
    struct AcceptAllVerifier;
    impl rustls::client::danger::ServerCertVerifier for AcceptAllVerifier {
        fn verify_server_cert(
            &self,
            _end_entity: &rustls_pki_types::CertificateDer<'_>,
            _intermediates: &[rustls_pki_types::CertificateDer<'_>],
            _server_name: &rustls::pki_types::ServerName<'_>,
            _ocsp: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
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

    let tls = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAllVerifier))
        .with_no_client_auth();
    let quic_tls = quinn::crypto::rustls::QuicClientConfig::try_from(tls)
        .map_err(|_| "invalid tls config".to_string())?;
    Ok(ClientConfig::new(Arc::new(quic_tls)))
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let identity = QuicIdentity::generate_self_signed("127.0.0.1").map_err(|e| e.to_string())?;
    let bind_addr = free_udp_addr();
    let server_cfg = build_server_config(&identity)?;
    let mut endpoint = Endpoint::server(server_cfg, bind_addr)
        .map_err(|_| "failed to bind endpoint".to_string())?;
    let client_cfg = build_insecure_client_config()?;
    endpoint.set_default_client_config(client_cfg);

    let peers: Arc<Mutex<Vec<SocketAddr>>> = Arc::new(Mutex::new(Vec::new()));

    println!("READY {} {}", bind_addr, hex_encode(&identity.cert_der));
    let _ = io::stdout().flush();

    let debug = std::env::var_os("VEIL_QUIC_DEBUG").is_some();
    while let Some(incoming) = endpoint.accept().await {
        let peers = Arc::clone(&peers);
        let endpoint = endpoint.clone();
        tokio::spawn(async move {
            match incoming.await {
                Ok(conn) => {
                    let remote = conn.remote_address();
                    if debug {
                        eprintln!("relay accepted {remote}");
                    }
                    if let Ok(mut list) = peers.lock() {
                        if !list.contains(&remote) {
                            list.push(remote);
                        }
                    }
                    while let Ok(mut recv) = conn.accept_uni().await {
                        match recv.read_to_end(128 * 1024).await {
                            Ok(bytes) => {
                                if debug {
                                    eprintln!("relay recv {} bytes from {remote}", bytes.len());
                                }
                                let peers = match peers.lock() {
                                    Ok(list) => list.clone(),
                                    Err(_) => Vec::new(),
                                };
                                for peer in peers {
                                    if peer == remote {
                                        continue;
                                    }
                                    if debug {
                                        eprintln!("relay connect to {peer}");
                                    }
                                    if let Ok(connecting) = endpoint.connect(peer, "127.0.0.1") {
                                        if let Ok(Ok(conn)) =
                                            tokio::time::timeout(Duration::from_secs(5), connecting)
                                                .await
                                        {
                                            if debug {
                                                eprintln!("relay connected to {peer}");
                                            }
                                            if let Ok(mut stream) = conn.open_uni().await {
                                                let _ = stream.write_all(&bytes).await;
                                                let _ = stream.finish();
                                                let _ = stream.stopped().await;
                                                if debug {
                                                    eprintln!("relay sent to {peer}");
                                                }
                                            }
                                        } else if debug {
                                            eprintln!("relay connect timeout to {peer}");
                                        }
                                    } else if debug {
                                        eprintln!("relay connect failed to {peer}");
                                    }
                                }
                            }
                            Err(_) => break,
                        }
                    }
                }
                Err(_) => {}
            }
        });
    }

    Ok(())
}
