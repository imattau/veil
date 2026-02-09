import { describe, expect, it } from "vitest";
import { parseDiscoveryInput } from "../src/discovery_parse";
import { contactBundleToQr, generateIdentity, signContactBundle } from "../src/identity";

describe("discovery parse", () => {
  it("handles valid contact bundle", async () => {
    const identity = await generateIdentity();
    const bundle = {
      version: 1,
      pubkeyHex: Buffer.from(identity.publicKey).toString("hex"),
      quicCertHex: "aa",
      endpoints: ["quic://127.0.0.1:9000"],
      createdAt: 1,
    };
    const signed = await signContactBundle(bundle, identity);
    const result = parseDiscoveryInput(contactBundleToQr(signed));
    expect(result).not.toBeNull();
    expect(result!.contactBundle).toBeTruthy();
  });

  it("handles vps link", () => {
    const value =
      "veil://vps?ws=wss%3A%2F%2Fnode.example%2Fws&quic=quic%3A%2F%2Fnode.example%3A4444&cert=aa&peer=p1&tag=t1";
    const result = parseDiscoveryInput(value);
    expect(result).not.toBeNull();
    expect(result!.wsEndpoints[0]).toBe("wss://node.example/ws");
    expect(result!.quicEndpoints[0]).toBe("quic://node.example:4444");
    expect(result!.quicCertHex).toBe("aa");
    expect(result!.peers[0]).toBe("p1");
    expect(result!.tags[0]).toBe("t1");
    expect(result!.isVpsProfile).toBe(true);
  });

  it("handles ws/quic/peer/tag", () => {
    expect(parseDiscoveryInput("wss://node.example/ws")!.wsEndpoints.length).toBe(1);
    expect(parseDiscoveryInput("quic://node.example:4444")!.quicEndpoints.length).toBe(1);
    expect(parseDiscoveryInput("peer:abc")!.peers[0]).toBe("abc");
    expect(parseDiscoveryInput("tag:dead")!.tags[0]).toBe("dead");
  });
});
