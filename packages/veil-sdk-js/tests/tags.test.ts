import { describe, expect, test } from "vitest";

import {
  deriveChannelFeedTagHex,
  deriveChannelNamespace,
  normalizeChannelId,
} from "../src/tags";

describe("channel-scoped tag helpers", () => {
  test("normalizes channel identifiers", () => {
    expect(normalizeChannelId(" General ")).toBe("general");
    expect(normalizeChannelId("")).toBe("general");
  });

  test("derives deterministic channel namespaces with stable vectors", () => {
    expect(deriveChannelNamespace(7, "general")).toBe(8562);
    expect(deriveChannelNamespace(7, "dev")).toBe(38851);
    expect(deriveChannelNamespace(7, "media")).toBe(57098);
    expect(deriveChannelNamespace(7, " General ")).toBe(8562);
  });

  test("derives channel feed tags that differ by channel", async () => {
    const key = "11".repeat(32);
    const general = await deriveChannelFeedTagHex(key, 7, "general");
    const dev = await deriveChannelFeedTagHex(key, 7, "dev");
    const generalNormalized = await deriveChannelFeedTagHex(key, 7, " General ");

    expect(general).not.toBe(dev);
    expect(general).toBe(generalNormalized);
  });
});
