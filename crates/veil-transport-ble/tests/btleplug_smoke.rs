#[cfg(feature = "btleplug")]
#[test]
fn btleplug_link_smoke() {
    if std::env::var("VEIL_BLE_E2E").ok().as_deref() != Some("1") {
        eprintln!("set VEIL_BLE_E2E=1 to run btleplug smoke test");
        return;
    }

    let link = veil_transport_ble::btleplug_backend::BtleplugLink::spawn(
        veil_transport_ble::btleplug_backend::BtleplugLinkConfig::default(),
    );

    assert!(
        link.is_ok(),
        "btleplug link should initialize when BLE is available"
    );
}

#[cfg(not(feature = "btleplug"))]
#[test]
fn btleplug_link_smoke() {
    eprintln!("enable feature veil-transport-ble/btleplug to run this test");
}
