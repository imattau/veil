import { describe, expect, test, vi } from "vitest";

vi.mock("../src/codec", () => ({
  decodeShardMeta: async (bytes: Uint8Array) => {
    if (bytes[0] === 0xff) {
      throw new Error("tampered shard");
    }
    return {
      version: 1,
      namespace: 1,
      epoch: 1,
      tagHex: "11".repeat(32),
      objectRootHex: "22".repeat(32),
      k: 6,
      n: 10,
      index: 0,
      payloadLen: bytes.length,
    };
  },
}));

import { VeilClient, type LaneAdapter } from "../src/client";

class TestLane implements LaneAdapter {
  readonly inbox: Array<{ peer: string; bytes: Uint8Array }> = [];
  readonly sent: Array<{ peer: string; bytes: Uint8Array }> = [];

  async send(peer: string, bytes: Uint8Array): Promise<void> {
    this.sent.push({ peer, bytes: new Uint8Array(bytes) });
  }

  async recv(): Promise<{ peer: string; bytes: Uint8Array } | null> {
    const msg = this.inbox.shift();
    return msg ?? null;
  }

  enqueue(peer: string, bytes: Uint8Array): void {
    this.inbox.push({ peer, bytes });
  }
}

describe("browser-like e2e behavior", () => {
  test("handles loss, drops duplicates, and ignores tampered payloads", async () => {
    const fastLane = new TestLane();
    const fallbackLane = new TestLane();

    let received = 0;
    let duplicates = 0;
    let malformed = 0;

    const client = new VeilClient(fastLane, fallbackLane, {
      onShard: () => {
        received += 1;
      },
      onIgnoredDuplicate: () => {
        duplicates += 1;
      },
      onIgnoredMalformed: () => {
        malformed += 1;
      },
    });

    client.subscribe("11".repeat(32));
    client.setForwardPeers(["peer-a", "peer-b"]);

    const shard = new Uint8Array([1, 2, 3, 4]);
    // "Loss" is simulated by only delivering this one shard.
    fastLane.enqueue("origin", shard);
    await client.tick();

    // Duplicate delivery should be ignored.
    fastLane.enqueue("origin", shard);
    await client.tick();

    // Tampered payload should be ignored without failing the runtime tick.
    fastLane.enqueue("origin", new Uint8Array([0xff, 0x01]));
    await client.tick();

    expect(received).toBe(1);
    expect(duplicates).toBe(1);
    expect(malformed).toBe(1);
  });
});
