import { describe, expect, test } from "vitest";

import { LocalWotPolicy, rankFeedItemsByTrust } from "../src/wot";

const hex = (byte: string): string => byte.repeat(64);

describe("LocalWotPolicy", () => {
  test("classifies trusted and blocked overrides", () => {
    const policy = new LocalWotPolicy();
    const trusted = hex("1");
    const blocked = hex("2");

    policy.trust(trusted);
    policy.block(blocked);

    expect(policy.classifyPublisher(trusted, 100)).toBe("trusted");
    expect(policy.classifyPublisher(blocked, 100)).toBe("blocked");
  });

  test("produces known tier from threshold endorsements", () => {
    const policy = new LocalWotPolicy();
    const t1 = hex("a");
    const t2 = hex("b");
    const publisher = hex("c");

    policy.trust(t1);
    policy.trust(t2);
    policy.addEndorsement(t1, publisher, 95);
    policy.addEndorsement(t2, publisher, 96);

    const explanation = policy.explainPublisher(publisher, 100);
    expect(explanation.score).toBeGreaterThan(0);
    expect(["known", "trusted"]).toContain(explanation.tier);
  });

  test("round-trips policy snapshots via json", () => {
    const policy = new LocalWotPolicy();
    const trusted = hex("d");
    const trusted2 = hex("9");
    const publisher = hex("e");

    policy.trust(trusted);
    policy.trust(trusted2);
    policy.addEndorsement(trusted, publisher, 40);
    policy.addEndorsement(trusted2, publisher, 42);
    const imported = LocalWotPolicy.importJson(policy.exportJson());

    expect(imported.classifyPublisher(trusted, 50)).toBe("trusted");
    expect(imported.scorePublisher(publisher, 50)).toBeGreaterThan(0);
  });
});

describe("rankFeedItemsByTrust", () => {
  test("sorts by trust tier then recency and filters blocked", () => {
    const policy = new LocalWotPolicy();
    const trusted = hex("f");
    const knownPublisher = hex("3");
    const blocked = hex("4");
    const t1 = hex("5");
    const t2 = hex("6");

    policy.trust(trusted);
    policy.trust(t1);
    policy.trust(t2);
    policy.addEndorsement(t1, knownPublisher, 70);
    policy.addEndorsement(t2, knownPublisher, 72);
    policy.block(blocked);

    const ranked = rankFeedItemsByTrust(
      [
        { publisher: blocked, createdAtStep: 99, id: "blocked" },
        { publisher: knownPublisher, createdAtStep: 95, id: "known" },
        { publisher: trusted, createdAtStep: 60, id: "trusted" },
        { publisher: hex("7"), createdAtStep: 98, id: "unknown" },
      ],
      policy,
      100,
    );

    expect(ranked.map((item) => item.id)).toEqual(["trusted", "known", "unknown"]);
  });
});
