import type { LaneAdapter } from "./client";

export class InMemoryLaneAdapter implements LaneAdapter {
  private readonly inbox: Array<{ peer: string; bytes: Uint8Array }> = [];

  async send(peer: string, bytes: Uint8Array): Promise<void> {
    this.inbox.push({ peer, bytes: new Uint8Array(bytes) });
  }

  async recv(): Promise<{ peer: string; bytes: Uint8Array } | null> {
    const next = this.inbox.shift();
    return next ?? null;
  }

  enqueue(peer: string, bytes: Uint8Array): void {
    this.inbox.push({ peer, bytes });
  }
}

export interface WebSocketLaneOptions {
  url: string;
  peerId: string;
}

// Browser/React-Native compatible websocket lane scaffold.
export class WebSocketLaneAdapter implements LaneAdapter {
  private readonly socket: WebSocket;
  private readonly inbox: Array<{ peer: string; bytes: Uint8Array }> = [];

  constructor(private readonly options: WebSocketLaneOptions) {
    this.socket = new WebSocket(options.url);
    this.socket.binaryType = "arraybuffer";
    this.socket.addEventListener("message", (evt) => {
      const bytes = new Uint8Array(evt.data as ArrayBuffer);
      this.inbox.push({ peer: options.peerId, bytes });
    });
  }

  async send(_peer: string, bytes: Uint8Array): Promise<void> {
    this.socket.send(bytes);
  }

  async recv(): Promise<{ peer: string; bytes: Uint8Array } | null> {
    const next = this.inbox.shift();
    return next ?? null;
  }
}
