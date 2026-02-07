import type { LaneAdapter, TransportHealthSnapshot } from "./client";

export type MultiLaneSendMode = "round_robin" | "broadcast";

export class MultiLaneAdapter implements LaneAdapter {
  private readonly lanes: LaneAdapter[];
  private readonly sendMode: MultiLaneSendMode;
  private sendIndex = 0;
  private recvIndex = 0;

  constructor(lanes: LaneAdapter[], sendMode: MultiLaneSendMode = "round_robin") {
    this.lanes = lanes.slice();
    this.sendMode = sendMode;
  }

  async send(peer: string, bytes: Uint8Array): Promise<void> {
    if (this.lanes.length === 0) return;
    if (this.sendMode === "broadcast") {
      for (const lane of this.lanes) {
        await lane.send(peer, bytes);
      }
      return;
    }
    const lane = this.lanes[this.sendIndex % this.lanes.length];
    this.sendIndex = (this.sendIndex + 1) % this.lanes.length;
    await lane.send(peer, bytes);
  }

  async recv(): Promise<{ peer: string; bytes: Uint8Array } | null> {
    if (this.lanes.length === 0) return null;
    for (let i = 0; i < this.lanes.length; i += 1) {
      const index = (this.recvIndex + i) % this.lanes.length;
      const msg = await this.lanes[index].recv();
      if (msg) {
        this.recvIndex = (index + 1) % this.lanes.length;
        return msg;
      }
    }
    return null;
  }

  healthSnapshot(): TransportHealthSnapshot {
    let outboundQueued = 0;
    let outboundSendOk = 0;
    let outboundSendErr = 0;
    let inboundReceived = 0;
    let inboundDropped = 0;
    let reconnectAttempts = 0;
    for (const lane of this.lanes) {
      const snapshot = lane.healthSnapshot?.();
      if (!snapshot) continue;
      outboundQueued += snapshot.outboundQueued;
      outboundSendOk += snapshot.outboundSendOk;
      outboundSendErr += snapshot.outboundSendErr;
      inboundReceived += snapshot.inboundReceived;
      inboundDropped += snapshot.inboundDropped;
      reconnectAttempts += snapshot.reconnectAttempts;
    }
    return {
      outboundQueued,
      outboundSendOk,
      outboundSendErr,
      inboundReceived,
      inboundDropped,
      reconnectAttempts,
    };
  }
}
