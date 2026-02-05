import { describe, expect, test, vi } from "vitest";

import { WebSocketLaneAdapter } from "../src/transports";
import { WebRtcLaneAdapter, type WebRtcDataChannelLike } from "../src/webrtc";

type Listener = (() => void) | ((event: { data: unknown }) => void);

class FakeSocket {
  readyState = 0;
  binaryType = "";
  sent: Uint8Array[] = [];
  private readonly listeners = new Map<string, Listener[]>();

  send(data: Uint8Array): void {
    this.sent.push(new Uint8Array(data));
  }

  close(): void {
    this.readyState = 3;
    this.emit("close");
  }

  addEventListener(type: string, listener: Listener): void {
    const list = this.listeners.get(type) ?? [];
    list.push(listener);
    this.listeners.set(type, list);
  }

  emit(type: string, event: { data: unknown } = { data: undefined }): void {
    for (const fn of this.listeners.get(type) ?? []) {
      (fn as (e: { data: unknown }) => void)(event);
    }
  }

  open(): void {
    this.readyState = 1;
    this.emit("open");
  }
}

class FakeRtcChannel implements WebRtcDataChannelLike {
  readyState = "connecting";
  sent: Uint8Array[] = [];
  private readonly listeners = new Map<string, Listener[]>();

  send(data: Uint8Array): void {
    this.sent.push(new Uint8Array(data));
  }

  close(): void {
    this.readyState = "closed";
    this.emit("close");
  }

  addEventListener(type: string, listener: Listener): void {
    const list = this.listeners.get(type) ?? [];
    list.push(listener);
    this.listeners.set(type, list);
  }

  emit(type: string, event: { data: unknown } = { data: undefined }): void {
    for (const fn of this.listeners.get(type) ?? []) {
      (fn as (e: { data: unknown }) => void)(event);
    }
  }

  open(): void {
    this.readyState = "open";
    this.emit("open");
  }
}

describe("lane adapters", () => {
  test("WebSocketLaneAdapter buffers before open and flushes on open", async () => {
    const socket = new FakeSocket();
    const lane = new WebSocketLaneAdapter({
      url: "wss://example.invalid",
      peerId: "p1",
      webSocketFactory: () => socket,
    });

    await lane.send("peer-x", new Uint8Array([1, 2, 3]));
    expect(socket.sent).toHaveLength(0);

    socket.open();
    expect(socket.sent).toHaveLength(1);
    expect(socket.sent[0]).toEqual(new Uint8Array([1, 2, 3]));

    lane.close();
  });

  test("WebSocketLaneAdapter reconnects with backoff after close", async () => {
    vi.useFakeTimers();
    const sockets: FakeSocket[] = [];
    const lane = new WebSocketLaneAdapter({
      url: "wss://example.invalid",
      peerId: "p1",
      reconnectInitialMs: 10,
      reconnectMaxMs: 20,
      reconnectBackoffMultiplier: 2,
      webSocketFactory: () => {
        const s = new FakeSocket();
        sockets.push(s);
        return s;
      },
    });

    expect(sockets).toHaveLength(1);
    sockets[0].emit("close");

    await vi.advanceTimersByTimeAsync(10);
    expect(sockets).toHaveLength(2);

    lane.close();
    vi.useRealTimers();
  });

  test("WebRtcLaneAdapter buffers before open and flushes on open", async () => {
    const channel = new FakeRtcChannel();
    const lane = new WebRtcLaneAdapter({
      peerId: "rtc-peer",
      createChannel: async () => channel,
    });

    await lane.send("p", new Uint8Array([4, 5, 6]));
    expect(channel.sent).toHaveLength(0);

    channel.open();
    expect(channel.sent).toHaveLength(1);
    expect(channel.sent[0]).toEqual(new Uint8Array([4, 5, 6]));

    lane.close();
  });
});
