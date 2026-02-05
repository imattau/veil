import type { FeedModel } from "./types";

const KEY_PREFIX = "veil-demo-feed:";

function storageKey(channelId: string): string {
  return `${KEY_PREFIX}${channelId}`;
}

export function loadFeedModel(channelId: string): FeedModel | null {
  try {
    const raw = globalThis.localStorage?.getItem(storageKey(channelId));
    if (!raw) {
      return null;
    }
    const parsed = JSON.parse(raw) as FeedModel;
    if (!parsed || typeof parsed !== "object") {
      return null;
    }
    return parsed;
  } catch {
    return null;
  }
}

export function saveFeedModel(model: FeedModel): void {
  try {
    globalThis.localStorage?.setItem(storageKey(model.channelId), JSON.stringify(model));
  } catch {
    // Best-effort demo persistence.
  }
}
