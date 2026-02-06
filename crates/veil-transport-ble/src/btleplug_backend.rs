use std::collections::HashMap;
use std::sync::{mpsc, Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use btleplug::api::{Central, CentralEvent, Manager as _, Peripheral as _, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use futures_util::StreamExt;
use tokio::sync::{mpsc as tokio_mpsc, oneshot};
use uuid::Uuid;

use crate::protocol::{BleFrame, BLE_SHARD_CHAR_UUID};
use crate::{BleLink, BlePeer};

#[derive(Debug, Clone)]
pub struct BtleplugLinkConfig {
    pub scan_interval: Duration,
    pub connect_timeout: Duration,
    pub outbound_queue_capacity: usize,
    pub inbound_queue_capacity: usize,
    pub allowlist: Vec<String>,
}

impl Default for BtleplugLinkConfig {
    fn default() -> Self {
        Self {
            scan_interval: Duration::from_secs(2),
            connect_timeout: Duration::from_secs(6),
            outbound_queue_capacity: 1024,
            inbound_queue_capacity: 4096,
            allowlist: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub enum BtleplugLinkError {
    ManagerUnavailable,
    AdapterUnavailable,
    WorkerFailed,
}

#[derive(Debug)]
pub struct BtleplugLink {
    outbound_tx: tokio_mpsc::Sender<(BlePeer, BleFrame)>,
    inbound_rx: mpsc::Receiver<(BlePeer, BleFrame)>,
    shutdown_tx: Option<oneshot::Sender<()>>,
    worker: Option<JoinHandle<()>>,
    mtu: usize,
}

impl BtleplugLink {
    pub fn spawn(config: BtleplugLinkConfig) -> Result<Self, BtleplugLinkError> {
        let (outbound_tx, outbound_rx) =
            tokio_mpsc::channel::<(BlePeer, BleFrame)>(config.outbound_queue_capacity);
        let (inbound_tx, inbound_rx) =
            mpsc::sync_channel::<(BlePeer, BleFrame)>(config.inbound_queue_capacity);
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();
        let config_clone = config.clone();

        let worker = thread::spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(rt) => rt,
                Err(_) => return,
            };
            runtime.block_on(run_worker(config_clone, outbound_rx, inbound_tx, shutdown_rx));
        });

        Ok(Self {
            outbound_tx,
            inbound_rx,
            shutdown_tx: Some(shutdown_tx),
            worker: Some(worker),
            mtu: 180,
        })
    }
}

impl BleLink for BtleplugLink {
    type Error = BtleplugLinkError;

    fn send_frame(&mut self, peer: &BlePeer, frame: &BleFrame) -> Result<(), Self::Error> {
        self.outbound_tx
            .try_send((peer.clone(), frame.clone()))
            .map_err(|_| BtleplugLinkError::WorkerFailed)
    }

    fn recv_frame(&mut self) -> Option<(BlePeer, BleFrame)> {
        self.inbound_rx.try_recv().ok()
    }

    fn mtu(&self) -> usize {
        self.mtu
    }
}

impl Drop for BtleplugLink {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

async fn run_worker(
    config: BtleplugLinkConfig,
    mut outbound_rx: tokio_mpsc::Receiver<(BlePeer, BleFrame)>,
    inbound_tx: mpsc::SyncSender<(BlePeer, BleFrame)>,
    mut shutdown_rx: oneshot::Receiver<()>,
) {
    let manager = match Manager::new().await {
        Ok(m) => m,
        Err(_) => return,
    };
    let adapters = match manager.adapters().await {
        Ok(a) => a,
        Err(_) => return,
    };
    let adapter = match adapters.into_iter().next() {
        Some(a) => a,
        None => return,
    };

    let peripherals = Arc::new(Mutex::new(HashMap::<String, Peripheral>::new()));
    let notify_tasks = Arc::new(Mutex::new(HashMap::<String, tokio::task::JoinHandle<()>>::new()));

    if adapter
        .start_scan(btleplug::api::ScanFilter::default())
        .await
        .is_err()
    {
        return;
    }

    let mut events = match adapter.events().await {
        Ok(e) => e,
        Err(_) => return,
    };

    loop {
        tokio::select! {
            _ = &mut shutdown_rx => {
                break;
            }
            maybe_event = events.next() => {
                if let Some(event) = maybe_event {
                    handle_event(&adapter, &config, event, &peripherals, &notify_tasks, inbound_tx.clone()).await;
                }
            }
            Some((peer, frame)) = outbound_rx.recv() => {
                let encoded = frame.encode();
                send_to_peer(&adapter, &config, &peripherals, &peer, &encoded).await;
            }
            _ = tokio::time::sleep(config.scan_interval) => {
                let _ = adapter.start_scan(btleplug::api::ScanFilter::default()).await;
            }
        }
    }
}

async fn handle_event(
    adapter: &Adapter,
    config: &BtleplugLinkConfig,
    event: CentralEvent,
    peripherals: &Arc<Mutex<HashMap<String, Peripheral>>>,
    notify_tasks: &Arc<Mutex<HashMap<String, tokio::task::JoinHandle<()>>>>,
    inbound_tx: mpsc::SyncSender<(BlePeer, BleFrame)>,
) {
    let id = match event {
        CentralEvent::DeviceDiscovered(id) => id,
        CentralEvent::DeviceUpdated(id) => id,
        CentralEvent::DeviceConnected(id) => id,
        _ => return,
    };

    let peripheral = match adapter.peripheral(&id).await {
        Ok(p) => p,
        Err(_) => return,
    };

    let addr = id.to_string();
    if !config.allowlist.is_empty() && !config.allowlist.iter().any(|a| a == &addr) {
        return;
    }

    if peripheral.connect().await.is_err() {
        return;
    }
    let _ = peripheral.discover_services().await;

    peripherals.lock().unwrap().insert(addr.clone(), peripheral.clone());

    if notify_tasks.lock().unwrap().contains_key(&addr) {
        return;
    }

    let inbound = inbound_tx.clone();
    let addr_clone = addr.clone();
    let handle = tokio::spawn(async move {
        if subscribe_notifications(&peripheral).await.is_err() {
            return;
        }
        if let Ok(mut notifications) = peripheral.notifications().await {
            while let Some(data) = notifications.next().await {
                if let Some(frame) = BleFrame::decode(&data.value) {
                    let peer = BlePeer::new(addr_clone.clone());
                    let _ = inbound.try_send((peer, frame));
                }
            }
        }
    });

    notify_tasks.lock().unwrap().insert(addr, handle);
}

async fn subscribe_notifications(peripheral: &Peripheral) -> Result<(), ()> {
    let char_uuid = match Uuid::parse_str(BLE_SHARD_CHAR_UUID) {
        Ok(u) => u,
        Err(_) => return Err(()),
    };
    let chars = peripheral.characteristics();
    let Some(ch) = chars.iter().find(|c| c.uuid == char_uuid).cloned() else {
        return Err(());
    };
    peripheral.subscribe(&ch).await.map_err(|_| ())
}

async fn send_to_peer(
    _adapter: &Adapter,
    _config: &BtleplugLinkConfig,
    peripherals: &Arc<Mutex<HashMap<String, Peripheral>>>,
    peer: &BlePeer,
    payload: &[u8],
) {
    let char_uuid = match Uuid::parse_str(BLE_SHARD_CHAR_UUID) {
        Ok(u) => u,
        Err(_) => return,
    };

    let peripheral = {
        peripherals.lock().unwrap().get(&peer.addr).cloned()
    };

    let Some(peripheral) = peripheral else { return; };
    let chars = peripheral.characteristics();
    let Some(ch) = chars.iter().find(|c| c.uuid == char_uuid).cloned() else {
        return;
    };
    let _ = peripheral.write(&ch, payload, WriteType::WithoutResponse).await;
}
