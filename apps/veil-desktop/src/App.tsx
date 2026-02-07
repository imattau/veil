import { useEffect, useMemo, useRef, useState } from "react";
import {
  InMemoryLaneAdapter,
  VeilClient,
  WebSocketLaneAdapter,
  createAutoFetchPlugin,
  createThreadContextPlugin,
  type LaneHealthSnapshot,
} from "@veil/sdk-js";

const DEFAULT_WS = "ws://127.0.0.1:9001";
const DEFAULT_TAG = "";

type LogEntry = {
  ts: string;
  message: string;
};

export default function App() {
  const [wsUrl, setWsUrl] = useState(DEFAULT_WS);
  const [peerId, setPeerId] = useState("desktop-client");
  const [forwardPeers, setForwardPeers] = useState("peer-a,peer-b");
  const [tagHex, setTagHex] = useState(DEFAULT_TAG);
  const [connected, setConnected] = useState(false);
  const [laneHealth, setLaneHealth] = useState<LaneHealthSnapshot | null>(null);
  const [logs, setLogs] = useState<LogEntry[]>([]);

  const clientRef = useRef<VeilClient | null>(null);
  const logBuffer = useRef<LogEntry[]>([]);

  const forwardPeerList = useMemo(
    () =>
      forwardPeers
        .split(",")
        .map((entry) => entry.trim())
        .filter(Boolean),
    [forwardPeers],
  );

  const pushLog = (message: string) => {
    const entry = { ts: new Date().toLocaleTimeString(), message };
    logBuffer.current = [entry, ...logBuffer.current].slice(0, 200);
    setLogs([...logBuffer.current]);
  };

  const lastEvent = logs[0]?.message ?? "No activity yet";
  const totalEvents = logs.length;

  useEffect(() => {
    return () => {
      clientRef.current?.stop();
      clientRef.current = null;
    };
  }, []);

  const connect = () => {
    if (clientRef.current) {
      clientRef.current.stop();
      clientRef.current = null;
    }

    const fastLane = new WebSocketLaneAdapter({
      url: wsUrl,
      peerId,
      autoReconnect: true,
    });
    const fallbackLane = new InMemoryLaneAdapter();

    const client = new VeilClient(
      fastLane,
      fallbackLane,
      {
        onShard: (peer) => {
          pushLog(`Shard received from ${peer}`);
        },
        onForward: (peer) => {
          pushLog(`Forwarded shard to ${peer}`);
        },
        onForwardError: (lane, peer, error) => {
          pushLog(`Forward error on ${lane} to ${peer}: ${String(error)}`);
        },
        onIgnoredMalformed: (peer) => {
          pushLog(`Ignored malformed payload from ${peer}`);
        },
        onIgnoredUnsubscribed: (tag) => {
          pushLog(`Ignored shard for unsubscribed tag ${tag}`);
        },
        onLaneHealth: (snapshot) => {
          setLaneHealth(snapshot);
        },
        onError: (error) => {
          pushLog(`Client error: ${String(error)}`);
        },
      },
      {
        pollIntervalMs: 50,
        adaptiveLaneScoring: true,
        plugins: [
          createAutoFetchPlugin({
            resolveTagForRoot: () => (tagHex ? tagHex : null),
          }),
          createThreadContextPlugin({
            resolveTagForRoot: () => (tagHex ? tagHex : null),
          }),
        ],
      },
    );

    if (tagHex) {
      client.subscribe(tagHex);
    }
    client.setForwardPeers(forwardPeerList);
    client.start();

    clientRef.current = client;
    setConnected(true);
    pushLog("Client started");
  };

  const disconnect = () => {
    clientRef.current?.stop();
    clientRef.current = null;
    setConnected(false);
    pushLog("Client stopped");
  };

  const updateSubscription = () => {
    const client = clientRef.current;
    if (!client) {
      return;
    }
    client.listSubscriptions().forEach((sub) => client.unsubscribe(sub));
    if (tagHex) {
      client.subscribe(tagHex);
      pushLog(`Subscribed to ${tagHex}`);
    }
  };

  return (
    <div className="app">
      <aside className="panel">
        <header>
          <div>
            <p className="eyebrow">VEIL Desktop</p>
            <h1>Edge Client Console</h1>
            <p className="subtitle">
              Multi-lane relay + cache monitor for local operators.
            </p>
          </div>
          <div className={`status ${connected ? "ok" : "idle"}`}>
            {connected ? "CONNECTED" : "IDLE"}
          </div>
        </header>

        <section className="section">
          <h2>Connection</h2>
          <label>
            WebSocket URL
            <input value={wsUrl} onChange={(e) => setWsUrl(e.target.value)} />
          </label>
          <label>
            Peer ID
            <input value={peerId} onChange={(e) => setPeerId(e.target.value)} />
          </label>
          <label>
            Forward peers (comma-separated)
            <input
              value={forwardPeers}
              onChange={(e) => setForwardPeers(e.target.value)}
            />
          </label>
          <label>
            Subscribe tag (hex)
            <input value={tagHex} onChange={(e) => setTagHex(e.target.value)} />
          </label>
          <div className="buttons">
            <button className="primary" onClick={connect} disabled={connected}>
              Start
            </button>
            <button className="ghost" onClick={disconnect} disabled={!connected}>
              Stop
            </button>
            <button onClick={updateSubscription} disabled={!connected}>
              Update Tag
            </button>
          </div>
        </section>

        <section className="section">
          <h2>Subscriptions</h2>
          <div className="pill-row">
            {clientRef.current?.listSubscriptions()?.length ? (
              clientRef.current
                ?.listSubscriptions()
                .map((sub) => (
                  <span className="pill" key={sub}>
                    {sub.slice(0, 10)}...
                  </span>
                ))
            ) : (
              <span className="muted">None yet</span>
            )}
          </div>
        </section>

        <section className="section">
          <h2>Lane Health</h2>
          <div className="health-grid">
            <div>
              <p>Fast Score</p>
              <strong>{laneHealth?.fast.score.toFixed(2) ?? "-"}</strong>
            </div>
            <div>
              <p>Fast Sends</p>
              <strong>{laneHealth?.fast.sends ?? 0}</strong>
            </div>
            <div>
              <p>Fast Receives</p>
              <strong>{laneHealth?.fast.receives ?? 0}</strong>
            </div>
            <div>
              <p>Fallback Score</p>
              <strong>{laneHealth?.fallback?.score.toFixed(2) ?? "-"}</strong>
            </div>
          </div>
        </section>

        <section className="section">
          <h2>Tips</h2>
          <ul>
            <li>Run a VEIL websocket relay or node and set its URL.</li>
            <li>Forward peers should match reachable peer IDs.</li>
            <li>Subscribe to the same tag your publisher uses.</li>
          </ul>
        </section>
      </aside>

      <main className="feed">
        <header className="feed-header">
          <div>
            <h2>Shard Activity</h2>
            <p>{connected ? "Live updates" : "Waiting for connection"}</p>
          </div>
          <div className="kpi">
            <div>
              <p>Total Events</p>
              <strong>{totalEvents}</strong>
            </div>
            <div>
              <p>Latest</p>
              <span>{lastEvent}</span>
            </div>
          </div>
        </header>
        <div className="log">
          {logs.length === 0 ? (
            <p className="muted">No activity yet.</p>
          ) : (
            logs.map((entry, idx) => (
              <div className="log-row" key={`${entry.ts}-${idx}`}>
                <span>{entry.ts}</span>
                <span>{entry.message}</span>
              </div>
            ))
          )}
        </div>
      </main>
    </div>
  );
}
