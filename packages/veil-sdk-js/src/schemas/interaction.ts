import { decodeAppEnvelope, encodeAppEnvelope } from "../app_schemas";
import type { AppEnvelope } from "../app_schemas";

export type ReactionV1 = {
  type: "reaction";
  version: 1;
  target_root: string;
  action_code: string;
  extensions?: Record<string, unknown>;
};

export type PollOptionV1 = {
  id: string;
  label: string;
};

export type PollV1 = {
  type: "poll";
  version: 1;
  question: string;
  options: PollOptionV1[];
  closes_at?: number;
  extensions?: Record<string, unknown>;
};

export type VoteV1 = {
  type: "vote";
  version: 1;
  poll_root: string;
  option_id: string;
  extensions?: Record<string, unknown>;
};

export function encodeReaction(reaction: ReactionV1): Uint8Array {
  return encodeAppEnvelope({
    type: reaction.type,
    version: reaction.version,
    payload: {
      target_root: reaction.target_root,
      action_code: reaction.action_code,
      extensions: reaction.extensions,
    },
  });
}

export function encodePoll(poll: PollV1): Uint8Array {
  return encodeAppEnvelope({
    type: poll.type,
    version: poll.version,
    payload: {
      question: poll.question,
      options: poll.options,
      closes_at: poll.closes_at,
      extensions: poll.extensions,
    },
  });
}

export function encodeVote(vote: VoteV1): Uint8Array {
  return encodeAppEnvelope({
    type: vote.type,
    version: vote.version,
    payload: {
      poll_root: vote.poll_root,
      option_id: vote.option_id,
      extensions: vote.extensions,
    },
  });
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return null;
  }
  return value as Record<string, unknown>;
}

export function decodeReaction(bytes: Uint8Array): ReactionV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "reaction" || envelope.version !== 1) {
    return null;
  }
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.target_root !== "string" || typeof payload.action_code !== "string") {
    return null;
  }
  return {
    type: "reaction",
    version: 1,
    target_root: payload.target_root,
    action_code: payload.action_code,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}

export function decodePoll(bytes: Uint8Array): PollV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "poll" || envelope.version !== 1) {
    return null;
  }
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.question !== "string" || !Array.isArray(payload.options)) {
    return null;
  }
  const options = payload.options
    .map((entry) => {
      if (!asRecord(entry)) {
        return null;
      }
      const id = entry.id;
      const label = entry.label;
      if (typeof id !== "string" || typeof label !== "string") {
        return null;
      }
      return { id, label };
    })
    .filter((entry): entry is PollOptionV1 => Boolean(entry));
  return {
    type: "poll",
    version: 1,
    question: payload.question,
    options,
    closes_at: typeof payload.closes_at === "number" ? payload.closes_at : undefined,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}

export function decodeVote(bytes: Uint8Array): VoteV1 | null {
  let envelope: AppEnvelope;
  try {
    envelope = decodeAppEnvelope(bytes);
  } catch {
    return null;
  }
  if (envelope.type !== "vote" || envelope.version !== 1) {
    return null;
  }
  if (!asRecord(envelope.payload)) {
    return null;
  }
  const payload = envelope.payload as Record<string, unknown>;
  if (typeof payload.poll_root !== "string" || typeof payload.option_id !== "string") {
    return null;
  }
  return {
    type: "vote",
    version: 1,
    poll_root: payload.poll_root,
    option_id: payload.option_id,
    extensions: asRecord(payload.extensions) ?? undefined,
  };
}
