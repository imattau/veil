import type { LaneAdapter, TransportHealthSnapshot } from "./client";

export interface WebRtcDataChannelLike {
  readonly readyState: string;
  send(data: Uint8Array): void;
  close(): void;
  addEventListener(type: "open", listener: () => void): void;
  addEventListener(type: "close", listener: () => void): void;
  addEventListener(type: "error", listener: () => void): void;
  addEventListener(
    type: "message",
    listener: (event: { data: unknown }) => void,
  ): void;
}

export interface WebRtcLaneOptions {
  peerId: string;
  createChannel: () => Promise<WebRtcDataChannelLike>;
  autoReconnect?: boolean;
  reconnectInitialMs?: number;
  reconnectMaxMs?: number;
  reconnectBackoffMultiplier?: number;
  maxBufferedMessages?: number;
}

type InboxMessage = { peer: string; bytes: Uint8Array };

function toBytes(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  throw new Error("Unsupported RTCDataChannel payload type");
}

export class WebRtcLaneAdapter implements LaneAdapter {
  private channel: WebRtcDataChannelLike | null = null;
  private readonly inbox: InboxMessage[] = [];
  private readonly sendBuffer: Uint8Array[] = [];
  private reconnectDelayMs: number;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private connectPromise: Promise<void> | null = null;
  private closed = false;
  private readonly metrics: TransportHealthSnapshot = {
    outboundQueued: 0,
    outboundSendOk: 0,
    outboundSendErr: 0,
    inboundReceived: 0,
    inboundDropped: 0,
    reconnectAttempts: 0,
  };

  constructor(private readonly options: WebRtcLaneOptions) {
    this.reconnectDelayMs = options.reconnectInitialMs ?? 250;
    this.connect();
  }

  private connect(): void {
    if (this.connectPromise || this.closed) {
      return;
    }

    this.connectPromise = this.options
      .createChannel()
      .then((channel) => {
        if (this.closed) {
          channel.close();
          return;
        }

        this.channel = channel;
        channel.addEventListener("open", () => {
          this.reconnectDelayMs = this.options.reconnectInitialMs ?? 250;
          this.flushBuffered();
        });
        channel.addEventListener("message", (event) => {
          try {
            const bytes = toBytes(event.data);
            this.inbox.push({ peer: this.options.peerId, bytes });
            this.metrics.inboundReceived += 1;
          } catch {
            // Ignore malformed payloads.
            this.metrics.inboundDropped += 1;
          }
        });
        channel.addEventListener("close", () => {
          this.channel = null;
          if (!this.closed && this.options.autoReconnect !== false) {
            this.scheduleReconnect();
          }
        });
        channel.addEventListener("error", () => {
          this.channel = null;
          if (!this.closed && this.options.autoReconnect !== false) {
            this.scheduleReconnect();
          }
        });
      })
      .catch(() => {
        if (!this.closed && this.options.autoReconnect !== false) {
          this.scheduleReconnect();
        }
      })
      .finally(() => {
        this.connectPromise = null;
      });
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer) {
      return;
    }
    const delay = this.reconnectDelayMs;
    const maxDelay = this.options.reconnectMaxMs ?? 10_000;
    const multiplier = this.options.reconnectBackoffMultiplier ?? 2;
    this.reconnectDelayMs = Math.min(Math.floor(delay * multiplier), maxDelay);
    this.metrics.reconnectAttempts += 1;
    this.reconnectTimer = setTimeout(() => {
      this.reconnectTimer = null;
      this.connect();
    }, delay);
  }

  private flushBuffered(): void {
    if (!this.channel || this.channel.readyState !== "open") {
      return;
    }
    while (this.sendBuffer.length > 0) {
      const bytes = this.sendBuffer.shift();
      if (!bytes) {
        break;
      }
      this.channel.send(bytes);
      this.metrics.outboundSendOk += 1;
    }
    this.metrics.outboundQueued = this.sendBuffer.length;
  }

  async send(_peer: string, bytes: Uint8Array): Promise<void> {
    if (this.closed) {
      throw new Error("WebRtcLaneAdapter is closed");
    }
    if (this.channel && this.channel.readyState === "open") {
      this.channel.send(bytes);
      this.metrics.outboundSendOk += 1;
      return;
    }

    const maxBufferedMessages = this.options.maxBufferedMessages ?? 256;
    if (this.sendBuffer.length >= maxBufferedMessages) {
      this.sendBuffer.shift();
      this.metrics.outboundSendErr += 1;
    }
    this.sendBuffer.push(new Uint8Array(bytes));
    this.metrics.outboundQueued = this.sendBuffer.length;
    this.connect();
  }

  async recv(): Promise<InboxMessage | null> {
    const next = this.inbox.shift();
    return next ?? null;
  }

  close(): void {
    this.closed = true;
    if (this.reconnectTimer) {
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = null;
    }
    if (this.channel) {
      this.channel.close();
      this.channel = null;
    }
  }

  healthSnapshot(): TransportHealthSnapshot {
    this.metrics.outboundQueued = this.sendBuffer.length;
    return { ...this.metrics };
  }
}
