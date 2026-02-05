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

  test("start/stop drives polling without manual ticks", async () => {
    vi.useFakeTimers();
    const fastLane = new StubLane();
    const fallbackLane = new StubLane();
    let received = 0;
    const client = new VeilClient(
      fastLane,
      fallbackLane,
      {
        onShard: () => {
          received += 1;
        },
      },
      {
        pollIntervalMs: 10,
      },
    );
    client.subscribe("11".repeat(32));
    fastLane.enqueue("origin", new Uint8Array([1, 3, 5, 7]));

    client.start();
    await vi.advanceTimersByTimeAsync(15);
    expect(received).toBe(1);

    client.stop();
    fastLane.enqueue("origin", new Uint8Array([2, 4, 6, 8]));
    await vi.advanceTimersByTimeAsync(20);
    expect(received).toBe(1);
    vi.useRealTimers();
  });

  test("prunes seen shard ids with bounded dedupe set", async () => {
    const fastLane = new StubLane();
    let duplicates = 0;
    let received = 0;
    const countingClient = new VeilClient(
      fastLane,
      undefined,
      {
        onIgnoredDuplicate: () => {
          duplicates += 1;
        },
        onShard: () => {
          received += 1;
        },
      },
      {
        maxSeenShardIds: 2,
        seenShardTtlMs: 60_000,
      },
    );
    countingClient.subscribe("11".repeat(32));

    const a = new Uint8Array([1, 1, 1]);
    const b = new Uint8Array([2, 2, 2]);
    const c = new Uint8Array([3, 3, 3]);

    fastLane.enqueue("origin", a);
    await countingClient.tick();
    fastLane.enqueue("origin", b);
    await countingClient.tick();
    fastLane.enqueue("origin", c);
    await countingClient.tick();
    fastLane.enqueue("origin", a);
    await countingClient.tick();

    expect(received).toBe(4);
    expect(duplicates).toBe(0);
  });

  test("throttles lane health callbacks", async () => {
    vi.useFakeTimers();
    const fastLane = new StubLane();
    const healthUpdates: number[] = [];
    const client = new VeilClient(
      fastLane,
      undefined,
      {
        onLaneHealth: (snapshot) => {
          healthUpdates.push(snapshot.fast.sends + snapshot.fast.receives);
        },
      },
      {
        fastFanout: 3,
        laneHealthEmitMs: 100,
      },
    );
    client.subscribe("11".repeat(32));
    client.setForwardPeers(["p1", "p2", "p3"]);
    fastLane.enqueue("origin", new Uint8Array([9, 9, 9]));

    await client.tick();
    // Immediate first emission only.
    expect(healthUpdates.length).toBe(1);
    await vi.advanceTimersByTimeAsync(120);
    // Deferred coalesced emission runs once.
    expect(healthUpdates.length).toBe(2);
    vi.useRealTimers();
  });
});
