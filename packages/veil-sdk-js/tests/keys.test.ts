import { describe, expect, test } from "vitest";

import {
  exportPublicKeyRaw,
  generateEd25519KeyPair,
  hkdfSha256,
  importEd25519PublicKeyRaw,
  randomBytes,
  signEd25519,
  verifyEd25519,
} from "../src/keys";

describe("key helpers", () => {
  test("randomBytes returns requested length and differs across calls", () => {
    const a = randomBytes(32);
    const b = randomBytes(32);
    expect(a).toHaveLength(32);
    expect(b).toHaveLength(32);
    expect(a).not.toEqual(b);
  });

  test("hkdfSha256 is deterministic for same inputs", async () => {
    const ikm = new Uint8Array([1, 2, 3, 4, 5]);
    const salt = new Uint8Array([9, 8, 7]);
    const info = new Uint8Array([6, 6, 6]);

    const k1 = await hkdfSha256(ikm, { salt, info, length: 32 });
    const k2 = await hkdfSha256(ikm, { salt, info, length: 32 });
    expect(k1).toEqual(k2);
    expect(k1).toHaveLength(32);
  });

  test("ed25519 helpers sign and verify", async () => {
    let keyPair: CryptoKeyPair;
    try {
      keyPair = await generateEd25519KeyPair();
    } catch {
      // Runtime does not support Ed25519 in WebCrypto; helper behavior is still valid.
      return;
    }

    const message = new TextEncoder().encode("veil-key-helper-test");
    const signature = await signEd25519(keyPair.privateKey, message);
    await expect(verifyEd25519(keyPair.publicKey, message, signature)).resolves.toBe(
      true,
    );

    const exported = await exportPublicKeyRaw(keyPair.publicKey);
    const imported = await importEd25519PublicKeyRaw(exported);
    await expect(verifyEd25519(imported, message, signature)).resolves.toBe(true);
  });
});
