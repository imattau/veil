import { describe, expect, it } from "vitest";
import {
  contactBundleImportFromQr,
  contactBundleToQr,
  generateIdentity,
  mergeContactBundleImport,
  signContactBundle,
} from "../src/identity";

describe("contact bundle import", () => {
  it("rejects invalid signature", async () => {
    const identity = await generateIdentity();
    const bundle = {
      version: 1,
      pubkeyHex: Buffer.from(identity.publicKey).toString("hex"),
      quicCertHex: "aa",
      endpoints: ["quic://127.0.0.1:5000"],
      createdAt: 1,
      signatureHex: "deadbeef",
    };
    const imported = await contactBundleImportFromQr(contactBundleToQr(bundle));
    expect(imported).toBeNull();
  });

  it("classifies endpoints", async () => {
    const identity = await generateIdentity();
    const bundle = {
      version: 1,
      pubkeyHex: Buffer.from(identity.publicKey).toString("hex"),
      quicCertHex: "aa",
      endpoints: [
        "quic://127.0.0.1:5000",
        "wss://node.example/ws",
        "https://node.example/ws",
        "custom://peer",
      ],
      createdAt: 1,
    };
    const signed = await signContactBundle(bundle, identity);
    const imported = await contactBundleImportFromQr(
      contactBundleToQr(signed),
    );
    expect(imported).not.toBeNull();
    expect(imported!.quicEndpoints.length).toBe(1);
    expect(imported!.wsEndpoints.length).toBe(2);
    expect(imported!.otherEndpoints.length).toBe(1);

    const merged = mergeContactBundleImport(imported!, {
      existingWs: ["wss://node.example/ws"],
      existingQuic: ["quic://127.0.0.1:5000"],
      existingQuicCerts: {},
    });
    expect(merged.wsEndpoints).toEqual([
      "wss://node.example/ws",
      "https://node.example/ws",
    ]);
    expect(merged.quicEndpoints).toEqual(["quic://127.0.0.1:5000"]);
    expect(merged.quicCertsByEndpoint["quic://127.0.0.1:5000"]).toBe("aa");
  });
});
