import { describe, expect, it } from "vitest";
import {
  contactBundleFromQr,
  contactBundleToQr,
  decodeContactBundle,
  encodeContactBundle,
  generateIdentity,
  signContactBundle,
  verifyContactBundle,
} from "../src/identity";

describe("contact bundle", () => {
  it("signs and verifies", async () => {
    const identity = await generateIdentity();
    const bundle = {
      version: 1,
      pubkeyHex: Buffer.from(identity.publicKey).toString("hex"),
      quicCertHex: "aa",
      endpoints: ["quic://127.0.0.1:5000"],
      createdAt: 1,
    };
    const signed = await signContactBundle(bundle, identity);
    expect(await verifyContactBundle(signed)).toBe(true);

    const encoded = encodeContactBundle(signed);
    const decoded = decodeContactBundle(encoded);
    expect(decoded.pubkeyHex).toBe(bundle.pubkeyHex);
    expect(await verifyContactBundle(decoded)).toBe(true);

    const qr = contactBundleToQr(signed);
    const fromQr = contactBundleFromQr(qr);
    expect(fromQr.pubkeyHex).toBe(bundle.pubkeyHex);
  });
});
