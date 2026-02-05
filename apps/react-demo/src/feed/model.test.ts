import { describe, expect, test } from "vitest";

import {
  applyFeedEnvelope,
  createDemoFeedModel,
  createEmptyFeedModel,
  diffFeedEnvelopes,
  publishPost,
} from "./model";

describe("feed directory flow", () => {
  test("publish rotates directory root and resolves new timeline head", () => {
    const base = createDemoFeedModel("general");
    const next = publishPost(base, "cc".repeat(32), "new demo post");
    expect(next.directoryRoot).not.toBe(base.directoryRoot);

    const previousDirectory = base.bundleStore[base.directoryRoot];
    const currentDirectory = next.bundleStore[next.directoryRoot];
    expect(previousDirectory?.kind).toBe("channel_directory");
    expect(currentDirectory?.kind).toBe("channel_directory");

    if (currentDirectory?.kind === "channel_directory") {
      const newHead = currentDirectory.postRoots[0];
      const newHeadBundle = next.bundleStore[newHead];
      expect(newHeadBundle?.kind).toBe("post");
      if (newHeadBundle?.kind === "post") {
        expect(newHeadBundle.text).toBe("new demo post");
      }
    }
  });

  test("replica converges by applying diff envelopes", () => {
    const initial = createDemoFeedModel("general");
    const replicaStart = createEmptyFeedModel("general");
    const seeded = diffFeedEnvelopes(replicaStart, initial).reduce(
      (acc, envelope) => applyFeedEnvelope(acc, envelope),
      replicaStart,
    );
    expect(seeded.directoryRoot).toBe(initial.directoryRoot);

    const next = publishPost(initial, "dd".repeat(32), "replicated post");
    const diff = diffFeedEnvelopes(initial, next);
    const converged = diff.reduce((acc, envelope) => applyFeedEnvelope(acc, envelope), seeded);
    expect(converged.directoryRoot).toBe(next.directoryRoot);
  });
});
