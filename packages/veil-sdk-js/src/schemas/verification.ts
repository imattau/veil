import { decodeAppEnvelope, encodeAppEnvelope } from "../app_schemas";
import type { AppEnvelope } from "../app_schemas";

export type TrustEndorsementV1 = {
  type: "trust";
  version: 1;
  target_pubkey: string;
  tier?: "trusted" | "known" | "unknown" | "muted" | "blocked";
  note?: string;
  extensions?: Record<string, unknown>;
};

export type RevocationV1 = {
  type: "revoke";
  version: 1;
  target_root: string;
  reason?: string;
  extensions?: Record<string, unknown>;
};

export type NamespacePolicyV1 = {
  type: "namespace_policy";
  version: 1;
  namespace: number;
  require_signed: boolean;
  max_object_size?: number;
  extensions?: Record<string, unknown>;
};

export type RelayHintV1 = {
  type: "relay_hint";
  version: 1;
  peer_id: string;
  address: string;
  lane?: string;
  latency_ms?: number;
  extensions?: Record<string, unknown>;
};

export function encodeTrustEndorsement(
  endorsement: TrustEndorsementV1,
): Uint8Array {
  return encodeAppEnvelope({
    type: endorsement.type,
    version: endorsement.version,
    payload: {
      target_pubkey: endorsement.target_pubkey,
      tier: endorsement.tier,
      note: endorsement.note,
      extensions: endorsement.extensions,
    },
  });
}

export function encodeRevocation(revocation: RevocationV1): Uint8Array {
  return encodeAppEnvelope({
    type: revocation.type,
    version: revocation.version,
    payload: {
      target_root: revocation.target_root,
      reason: revocation.reason,
      extensions: revocation.extensions,
    },
  });
}

export function encodeNamespacePolicy(policy: NamespacePolicyV1): Uint8Array {
  return encodeAppEnvelope({
    type: policy.type,
    version: policy.version,
    payload: {
      namespace: policy.namespace,
      require_signed: policy.require_signed,
      max_object_size: policy.max_object_size,
      extensions: policy.extensions,
    },
  });
}

export function encodeRelayHint(hint: RelayHintV1): Uint8Array {
  return encodeAppEnvelope({
    type: hint.type,
    version: hint.version,
    payload: {
      peer_id: hint.peer_id,
      address: hint.address,
      lane: hint.lane,
      latency_ms: hint.latency_ms,
      extensions: hint.extensions,
    },
  });
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

export function decodeTrustEndorsement(bytes: Uint8Array): TrustEndorsementV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "trust" || envelope.version !== 1) {
    return null;
  }
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.target_pubkey !== "string") {
    return null;
  }
  return {
    type: "trust",
    version: 1,
    target_pubkey: payload.target_pubkey,
    tier:
      typeof payload.tier === "string"
        ? (payload.tier as TrustEndorsementV1["tier"])
        : undefined,
    note: typeof payload.note === "string" ? payload.note : undefined,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}

export function decodeRevocation(bytes: Uint8Array): RevocationV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "revoke" || envelope.version !== 1) {
    return null;
  }
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.target_root !== "string") {
    return null;
  }
  return {
    type: "revoke",
    version: 1,
    target_root: payload.target_root,
    reason: typeof payload.reason === "string" ? payload.reason : undefined,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}

export function decodeNamespacePolicy(bytes: Uint8Array): NamespacePolicyV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "namespace_policy" || envelope.version !== 1) {
    return null;
  }
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.namespace !== "number" || typeof payload.require_signed !== "boolean") {
    return null;
  }
  return {
    type: "namespace_policy",
    version: 1,
    namespace: payload.namespace,
    require_signed: payload.require_signed,
    max_object_size: typeof payload.max_object_size === "number" ? payload.max_object_size : undefined,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}

export function decodeRelayHint(bytes: Uint8Array): RelayHintV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "relay_hint" || envelope.version !== 1) {
    return null;
  }
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.peer_id !== "string" || typeof payload.address !== "string") {
    return null;
  }
  return {
    type: "relay_hint",
    version: 1,
    peer_id: payload.peer_id,
    address: payload.address,
    lane: typeof payload.lane === "string" ? payload.lane : undefined,
    latency_ms: typeof payload.latency_ms === "number" ? payload.latency_ms : undefined,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}
