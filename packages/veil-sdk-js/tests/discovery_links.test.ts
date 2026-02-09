import { describe, expect, it } from "vitest";
import { buildVpsDiscoveryLink } from "../src/discovery_links";

describe("discovery links", () => {
  it("builds vps link with endpoints and cert", () => {
    const link = buildVpsDiscoveryLink({
      wsEndpoints: ["wss://node.example/ws", "wss://node2.example/ws"],
      quicEndpoint: "quic://node.example:4444",
      quicCertHex: "aa",
      peers: ["peer1", "peer2"],
      tags: ["tag1", "tag2"],
    });
    expect(link.startsWith("veil://vps?")).toBe(true);
    expect(link.includes("ws=wss%3A%2F%2Fnode.example%2Fws")).toBe(true);
    expect(link.includes("quic=quic%3A%2F%2Fnode.example%3A4444")).toBe(true);
    expect(link.includes("cert=aa")).toBe(true);
    expect(link.includes("peer=peer1")).toBe(true);
    expect(link.includes("tag=tag2")).toBe(true);
  });

  it("prefers certb64", () => {
    const link = buildVpsDiscoveryLink({
      wsEndpoints: ["wss://node.example/ws"],
      quicEndpoint: "quic://node.example:4444",
      quicCertHex: "aa",
      quicCertB64: "bb",
    });
    expect(link.includes("certb64=bb")).toBe(true);
    expect(link.includes("cert=aa")).toBe(false);
  });
});
