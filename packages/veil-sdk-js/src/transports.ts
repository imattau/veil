import type { LaneAdapter, TransportHealthSnapshot } from "./client";

export class InMemoryLaneAdapter implements LaneAdapter {
  private readonly inbox: Array<{ peer: string; bytes: Uint8Array }> = [];
  private readonly metrics: TransportHealthSnapshot = {
    outboundQueued: 0,
    outboundSendOk: 0,
    outboundSendErr: 0,
    inboundReceived: 0,
    inboundDropped: 0,
    reconnectAttempts: 0,
  };

  async send(peer: string, bytes: Uint8Array): Promise<void> {
    this.inbox.push({ peer, bytes: new Uint8Array(bytes) });
    this.metrics.outboundSendOk += 1;
    this.metrics.outboundQueued = this.inbox.length;
  }

  async recv(): Promise<{ peer: string; bytes: Uint8Array } | null> {
    const next = this.inbox.shift();
    if (next) {
      this.metrics.inboundReceived += 1;
      this.metrics.outboundQueued = this.inbox.length;
    }
    return next ?? null;
  }

  enqueue(peer: string, bytes: Uint8Array): void {
    this.inbox.push({ peer, bytes });
    this.metrics.outboundQueued = this.inbox.length;
  }

  healthSnapshot(): TransportHealthSnapshot {
    this.metrics.outboundQueued = this.inbox.length;
    return { ...this.metrics };
  }
}

export interface WebSocketLaneOptions {
  url: string;
  peerId: string;
  protocols?: string | string[];
  autoReconnect?: boolean;
  reconnectInitialMs?: number;
  reconnectMaxMs?: number;
  reconnectBackoffMultiplier?: number;
  maxBufferedMessages?: number;
  webSocketFactory?: (
    url: string,
    protocols?: string | string[],
  ) => WebSocketLike;
}

export interface WebSocketLike {
  readonly readyState: number;
  binaryType: string;
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

type InboxMessage = { peer: string; bytes: Uint8Array };

function resolveWebSocketFactory(
  options: WebSocketLaneOptions,
): (url: string, protocols?: string | string[]) => WebSocketLike {
  if (options.webSocketFactory) {
    return options.webSocketFactory;
  }

  const wsCtor = (globalThis as { WebSocket?: new (url: string, protocols?: string | string[]) => WebSocketLike })
    .WebSocket;
  if (!wsCtor) {
    throw new Error(
      "No WebSocket implementation available. Provide webSocketFactory in WebSocketLaneOptions.",
    );
  }

  return (url: string, protocols?: string | string[]) => new wsCtor(url, protocols);
}

function toBytes(value: unknown): Uint8Array {
  if (value instanceof Uint8Array) {
    return value;
  }
  if (value instanceof ArrayBuffer) {
    return new Uint8Array(value);
  }
  throw new Error("Unsupported websocket message payload type");
}

// Browser/Node/React-Native websocket lane with bounded buffering and reconnect/backoff.
export class WebSocketLaneAdapter implements LaneAdapter {
  private socket: WebSocketLike | null = null;
  private readonly inbox: InboxMessage[] = [];
  private readonly sendBuffer: Uint8Array[] = [];
  private reconnectDelayMs: number;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private closed = false;
  private readonly metrics: TransportHealthSnapshot = {
    outboundQueued: 0,
    outboundSendOk: 0,
    outboundSendErr: 0,
    inboundReceived: 0,
    inboundDropped: 0,
    reconnectAttempts: 0,
  };
  private readonly wsFactory: (
    url: string,
    protocols?: string | string[],
  ) => WebSocketLike;

  constructor(private readonly options: WebSocketLaneOptions) {
    this.wsFactory = resolveWebSocketFactory(options);
    this.reconnectDelayMs = options.reconnectInitialMs ?? 250;
    this.connect();
  }

  private connect(): void {
    const socket = this.wsFactory(this.options.url, this.options.protocols);
    this.socket = socket;
    socket.binaryType = "arraybuffer";
    socket.addEventListener("open", () => {
      this.reconnectDelayMs = this.options.reconnectInitialMs ?? 250;
      this.flushBuffered();
    });
    socket.addEventListener("message", (event) => {
      try {
        const bytes = toBytes(event.data);
        this.inbox.push({ peer: this.options.peerId, bytes });
        this.metrics.inboundReceived += 1;
      } catch {
        // Ignore malformed payloads from the websocket envelope layer.
        this.metrics.inboundDropped += 1;
      }
    });
    socket.addEventListener("close", () => {
      if (!this.closed && this.options.autoReconnect !== false) {
        this.scheduleReconnect();
      }
    });
    socket.addEventListener("error", () => {
      if (!this.closed && this.options.autoReconnect !== false) {
        this.scheduleReconnect();
      }
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
    if (!this.socket || this.socket.readyState !== 1) {
      return;
    }
    while (this.sendBuffer.length > 0) {
      const bytes = this.sendBuffer.shift();
      if (!bytes) {
        break;
      }
      this.socket.send(bytes);
      this.metrics.outboundSendOk += 1;
    }
    this.metrics.outboundQueued = this.sendBuffer.length;
  }

  async send(_peer: string, bytes: Uint8Array): Promise<void> {
    if (this.closed) {
      throw new Error("WebSocketLaneAdapter is closed");
    }
    if (this.socket && this.socket.readyState === 1) {
      this.socket.send(bytes);
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
    if (this.socket) {
      this.socket.close();
      this.socket = null;
    }
  }

  healthSnapshot(): TransportHealthSnapshot {
    this.metrics.outboundQueued = this.sendBuffer.length;
    return { ...this.metrics };
  }
}
