import { describe, expect, test } from "vitest";

import { MultiLaneAdapter } from "../src/multi_lane";
import type { LaneAdapter, TransportHealthSnapshot } from "../src/client";

class FakeLane implements LaneAdapter {
  public sent: Uint8Array[] = [];
  public inbox: { peer: string; bytes: Uint8Array }[] = [];
  constructor(public snapshot: TransportHealthSnapshot) {}

  async send(_peer: string, bytes: Uint8Array): Promise<void> {
    this.sent.push(bytes);
  }

  async recv(): Promise<{ peer: string; bytes: Uint8Array } | null> {
    return this.inbox.shift() ?? null;
  }

  healthSnapshot(): TransportHealthSnapshot {
    return this.snapshot;
  }
}

describe("MultiLaneAdapter", () => {
  test("round robin sends across lanes", async () => {
    const laneA = new FakeLane({
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    });
    const laneB = new FakeLane({
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    });
    const multi = new MultiLaneAdapter([laneA, laneB]);
    await multi.send("peer", new Uint8Array([1]));
    await multi.send("peer", new Uint8Array([2]));
    await multi.send("peer", new Uint8Array([3]));

    expect(laneA.sent.length).toBe(2);
    expect(laneB.sent.length).toBe(1);
  });

  test("broadcast sends to all lanes", async () => {
    const laneA = new FakeLane({
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    });
    const laneB = new FakeLane({
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    });
    const multi = new MultiLaneAdapter([laneA, laneB], "broadcast");
    await multi.send("peer", new Uint8Array([9]));
    expect(laneA.sent.length).toBe(1);
    expect(laneB.sent.length).toBe(1);
  });

  test("recv polls lanes in order", async () => {
    const laneA = new FakeLane({
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    });
    const laneB = new FakeLane({
      outboundQueued: 0,
      outboundSendOk: 0,
      outboundSendErr: 0,
      inboundReceived: 0,
      inboundDropped: 0,
      reconnectAttempts: 0,
    });
    laneB.inbox.push({ peer: "b", bytes: new Uint8Array([2]) });
    laneA.inbox.push({ peer: "a", bytes: new Uint8Array([1]) });
    const multi = new MultiLaneAdapter([laneA, laneB]);
    const first = await multi.recv();
    const second = await multi.recv();

    expect(first?.peer).toBe("a");
    expect(second?.peer).toBe("b");
  });

  test("aggregates health snapshots", () => {
    const laneA = new FakeLane({
      outboundQueued: 1,
      outboundSendOk: 2,
      outboundSendErr: 3,
      inboundReceived: 4,
      inboundDropped: 5,
      reconnectAttempts: 6,
    });
    const laneB = new FakeLane({
      outboundQueued: 2,
      outboundSendOk: 3,
      outboundSendErr: 4,
      inboundReceived: 5,
      inboundDropped: 6,
      reconnectAttempts: 7,
    });
    const multi = new MultiLaneAdapter([laneA, laneB]);
    const snapshot = multi.healthSnapshot();

    expect(snapshot.outboundQueued).toBe(3);
    expect(snapshot.outboundSendOk).toBe(5);
    expect(snapshot.outboundSendErr).toBe(7);
    expect(snapshot.inboundReceived).toBe(9);
    expect(snapshot.inboundDropped).toBe(11);
    expect(snapshot.reconnectAttempts).toBe(13);
  });
});
