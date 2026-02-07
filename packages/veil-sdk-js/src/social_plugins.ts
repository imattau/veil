import type { VeilClient, VeilClientPlugin } from "./client";
import { decodeAppEnvelope } from "./app_schemas";
import {
  decodeNamespacePolicy,
  decodeRelayHint,
  decodeRevocation,
  decodeTrustEndorsement,
} from "./schemas/verification";
import { decodePoll, decodeReaction, decodeVote } from "./schemas/interaction";

export type RevocationHandler = (targetRoot: string, revocationBytes: Uint8Array) => void;
export type ReactionHandler = (targetRoot: string, action: string) => void;
export type PollHandler = (pollRoot: string, pollBytes: Uint8Array) => void;
export type VoteHandler = (pollRoot: string, optionId: string) => void;
export type NamespacePolicyHandler = (
  namespace: number,
  requireSigned: boolean,
  maxObjectSize?: number,
) => void;
export type RelayHintHandler = (peerId: string, address: string) => void;
export type TrustEndorsementHandler = (targetPubkey: string) => void;

export function createRevocationPlugin(options: {
  onRevoke: RevocationHandler;
}): VeilClientPlugin {
  return {
    onObject(_client: VeilClient, _root: string, bytes: Uint8Array) {
      const revocation = decodeRevocation(bytes);
      if (!revocation) {
        return;
      }
      options.onRevoke(revocation.target_root, bytes);
    },
  };
}

export function createReactionAggregatorPlugin(options: {
  onReaction: ReactionHandler;
}): VeilClientPlugin {
  return {
    onObject(_client: VeilClient, _root: string, bytes: Uint8Array) {
      const reaction = decodeReaction(bytes);
      if (!reaction) {
        return;
      }
      options.onReaction(reaction.target_root, reaction.action_code);
    },
  };
}

export function createPollAggregatorPlugin(options: {
  onPoll: PollHandler;
  onVote?: VoteHandler;
}): VeilClientPlugin {
  return {
    onObject(_client: VeilClient, root: string, bytes: Uint8Array) {
      const poll = decodePoll(bytes);
      if (poll) {
        options.onPoll(root, bytes);
        return;
      }
      const vote = decodeVote(bytes);
      if (vote) {
        options.onVote?.(vote.poll_root, vote.option_id);
      }
    },
  };
}

export function createNamespacePolicyPlugin(options: {
  onPolicy: NamespacePolicyHandler;
}): VeilClientPlugin {
  return {
    onObject(_client: VeilClient, _root: string, bytes: Uint8Array) {
      const policy = decodeNamespacePolicy(bytes);
      if (!policy) {
        return;
      }
      options.onPolicy(
        policy.namespace,
        policy.require_signed,
        policy.max_object_size,
      );
    },
  };
}

export function createRelayHintPlugin(options: {
  onHint: RelayHintHandler;
}): VeilClientPlugin {
  return {
    onObject(_client: VeilClient, _root: string, bytes: Uint8Array) {
      const hint = decodeRelayHint(bytes);
      if (!hint) {
        return;
      }
      options.onHint(hint.peer_id, hint.address);
    },
  };
}

export function createTrustEndorsementPlugin(options: {
  onEndorsement: TrustEndorsementHandler;
}): VeilClientPlugin {
  return {
    onObject(_client: VeilClient, _root: string, bytes: Uint8Array) {
      const endorsement = decodeTrustEndorsement(bytes);
      if (!endorsement) {
        return;
      }
      options.onEndorsement(endorsement.target_pubkey);
    },
  };
}

export function isAppEnvelope(bytes: Uint8Array): boolean {
  try {
    decodeAppEnvelope(bytes);
    return true;
  } catch {
    return false;
  }
}
