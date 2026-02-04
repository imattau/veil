import { useState } from "react";
import { currentEpoch, deriveFeedTagHex, deriveRvTagHex } from "@veil/sdk-js";

export function App() {
  const [pubkey, setPubkey] = useState("11".repeat(32));
  const [namespace, setNamespace] = useState(7);
  const [result, setResult] = useState<string>("");

  async function deriveTags() {
    const now = Math.floor(Date.now() / 1000);
    const epoch = await currentEpoch(now);
    const feed = await deriveFeedTagHex(pubkey, namespace);
    const rv = await deriveRvTagHex(pubkey, epoch, namespace);
    setResult(`epoch=${epoch}\nfeed=${feed}\nrv=${rv}`);
  }

  return (
    <main style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", padding: 24 }}>
      <h1>VEIL Client-Native Demo</h1>
      <p>Derive VEIL tags in-browser via wasm.</p>
      <label>
        Pubkey hex (32 bytes):
        <input
          value={pubkey}
          onChange={(e) => setPubkey(e.target.value)}
          style={{ display: "block", width: "100%", marginTop: 8, marginBottom: 12 }}
        />
      </label>
      <label>
        Namespace:
        <input
          type="number"
          value={namespace}
          onChange={(e) => setNamespace(Number(e.target.value))}
          style={{ display: "block", marginTop: 8, marginBottom: 12 }}
        />
      </label>
      <button onClick={deriveTags}>Derive feed + rendezvous tags</button>
      <pre style={{ whiteSpace: "pre-wrap", marginTop: 16 }}>{result}</pre>
    </main>
  );
}
