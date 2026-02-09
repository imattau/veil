const DEFAULT_INFO = new TextEncoder().encode("veil/payload-key/v1");

type WebCryptoLike = {
  getRandomValues<T extends ArrayBufferView>(array: T): T;
  subtle: SubtleCrypto;
};

function toArrayBuffer(bytes: Uint8Array): ArrayBuffer {
  return bytes.buffer.slice(
    bytes.byteOffset,
    bytes.byteOffset + bytes.byteLength,
  ) as ArrayBuffer;
}

async function getWebCrypto(): Promise<WebCryptoLike> {
  const maybeCrypto = (globalThis as { crypto?: WebCryptoLike }).crypto;
  if (maybeCrypto?.subtle) {
    return maybeCrypto;
  }
  throw new Error("WebCrypto is unavailable in this runtime");
}

export function randomBytes(length: number): Uint8Array {
  if (!Number.isInteger(length) || length <= 0) {
    throw new Error("length must be a positive integer");
  }
  const maybeCrypto = (globalThis as { crypto?: WebCryptoLike }).crypto;
  if (!maybeCrypto) {
    throw new Error("crypto.getRandomValues is unavailable in this runtime");
  }
  const bytes = new Uint8Array(length);
  maybeCrypto.getRandomValues(bytes);
  return bytes;
}

export async function hkdfSha256(
  ikm: Uint8Array,
  options: {
    salt?: Uint8Array;
    info?: Uint8Array;
    length?: number;
  } = {},
): Promise<Uint8Array> {
  const crypto = await getWebCrypto();
  const salt = options.salt ?? new Uint8Array();
  const info = options.info ?? DEFAULT_INFO;
  const length = options.length ?? 32;
  if (!Number.isInteger(length) || length <= 0) {
    throw new Error("length must be a positive integer");
  }

  const key = await crypto.subtle.importKey("raw", toArrayBuffer(ikm), "HKDF", false, [
    "deriveBits",
  ]);
  const bits = await crypto.subtle.deriveBits(
    {
      name: "HKDF",
      hash: "SHA-256",
      salt: toArrayBuffer(salt),
      info: toArrayBuffer(info),
    },
    key,
    length * 8,
  );
  return new Uint8Array(bits);
}

export async function generateEd25519KeyPair(): Promise<CryptoKeyPair> {
  const crypto = await getWebCrypto();
  return crypto.subtle.generateKey(
    { name: "Ed25519" },
    true,
    ["sign", "verify"],
  ) as Promise<CryptoKeyPair>;
}

export async function signEd25519(
  privateKey: CryptoKey,
  message: Uint8Array,
): Promise<Uint8Array> {
  const crypto = await getWebCrypto();
  const signature = await crypto.subtle.sign(
    "Ed25519",
    privateKey,
    toArrayBuffer(message),
  );
  return new Uint8Array(signature);
}

export async function verifyEd25519(
  publicKey: CryptoKey,
  message: Uint8Array,
  signature: Uint8Array,
): Promise<boolean> {
  const crypto = await getWebCrypto();
  return crypto.subtle.verify(
    "Ed25519",
    publicKey,
    toArrayBuffer(signature),
    toArrayBuffer(message),
  );
}

export async function exportPublicKeyRaw(publicKey: CryptoKey): Promise<Uint8Array> {
  const crypto = await getWebCrypto();
  const bytes = await crypto.subtle.exportKey("raw", publicKey);
  return new Uint8Array(bytes);
}

export async function importEd25519PublicKeyRaw(bytes: Uint8Array): Promise<CryptoKey> {
  const crypto = await getWebCrypto();
  return crypto.subtle.importKey("raw", toArrayBuffer(bytes), { name: "Ed25519" }, true, [
    "verify",
  ]);
}

export async function exportEd25519PrivateKeyPkcs8(
  privateKey: CryptoKey,
): Promise<Uint8Array> {
  const crypto = await getWebCrypto();
  const bytes = await crypto.subtle.exportKey("pkcs8", privateKey);
  return new Uint8Array(bytes);
}

export async function importEd25519PrivateKeyPkcs8(
  bytes: Uint8Array,
): Promise<CryptoKey> {
  const crypto = await getWebCrypto();
  return crypto.subtle.importKey("pkcs8", toArrayBuffer(bytes), { name: "Ed25519" }, true, [
    "sign",
  ]);
}
