import { describe, expect, test, vi } from "vitest";

vi.mock("../src/codec", () => ({
  decodeShardMeta: async () => ({
    version: 1,
    namespace: 1,
    epoch: 1,
    tagHex: "11".repeat(32),
    objectRootHex: "22".repeat(32),
    k: 6,
    n: 10,
    index: 0,
    payloadLen: 16,
  }),
}));

import { VeilClient, type LaneAdapter } from "../src/client";

class StubLane implements LaneAdapter {
  readonly sent: Array<{ peer: string; bytes: Uint8Array }> = [];
  readonly inbox: Array<{ peer: string; bytes: Uint8Array }> = [];
  failSends = false;

  async send(peer: string, bytes: Uint8Array): Promise<void> {
    if (this.failSends) {
      throw new Error("send failed");
    }
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

describe("VeilClient lane scoring", () => {
  test("shifts forwarding away from failing fast lane", async () => {
    const fastLane = new StubLane();
    const fallbackLane = new StubLane();
    const healthSnapshots: number[] = [];

    const client = new VeilClient(
      fastLane,
      fallbackLane,
      {
        onLaneHealth: (snapshot) => {
          healthSnapshots.push(snapshot.fast.score);
        },
      },
      {
        fastFanout: 2,
        fallbackFanout: 2,
        adaptiveLaneScoring: true,
      },
    );

    const subscribedTag = "11".repeat(32);
    client.subscribe(subscribedTag);
    client.setForwardPeers(["p1", "p2", "p3", "p4"]);

    const payloadA = new Uint8Array([1, 2, 3, 4, 5]);

    const payloadB = new Uint8Array([6, 7, 8, 9, 10]);

    fastLane.failSends = true;
    fastLane.enqueue("origin", payloadA);
    await client.tick();

    const healthAfterFirst = client.getLaneHealth();
    expect(healthAfterFirst.fast.sendFailures).toBeGreaterThan(0);
    expect(healthAfterFirst.fast.score).toBeLessThan(0.5);

    fastLane.enqueue("origin", payloadB);
    await client.tick();

    // Adaptive scoring should push most forward attempts to fallback lane now.
    expect(fallbackLane.sent.length).toBeGreaterThan(fastLane.sent.length);
    expect(healthSnapshots.length).toBeGreaterThan(0);
  });
});
