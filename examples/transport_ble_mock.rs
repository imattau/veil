use veil_transport_ble::{BleAdapter, BleAdapterConfig, BlePeer, MockBleLink};

fn main() {
    let link_a = MockBleLink::with_mtu(64);
    let link_b = MockBleLink::with_mtu(64);

    let mut adapter_a = BleAdapter::new(link_a, BleAdapterConfig::default());
    let mut adapter_b = BleAdapter::new(link_b, BleAdapterConfig::default());

    let peer_a = BlePeer::new("ble-a");
    let peer_b = BlePeer::new("ble-b");

    let payload = vec![0x5Au8; 256];
    adapter_a.send(&peer_b, &payload).expect("send should succeed");

    let outbound = adapter_a.link_mut().take_outbound();
    for (_, frame) in outbound {
        adapter_b.link_mut().enqueue_inbound(peer_a.clone(), frame);
    }

    if let Some((peer, bytes)) = adapter_b.recv() {
        println!(
            "received {} bytes from {} (ok={})",
            bytes.len(),
            peer.addr,
            bytes == payload
        );
    } else {
        println!("no payload received");
    }
}
