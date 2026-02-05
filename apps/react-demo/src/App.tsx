import { useEffect, useMemo, useRef, useState } from "react";
import {
  currentEpoch,
  deriveFeedTagHex,
  InMemoryLaneAdapter,
  VeilClient,
} from "@veil/sdk-js";

import {
  applyFeedEnvelope,
  createDemoFeedModel,
  createEmptyFeedModel,
  diffFeedEnvelopes,
  feedEnvelopesFromModel,
  publishPost,
} from "./feed/model";
import { loadFeedModel, saveFeedModel } from "./feed/persistence";
import { decodeFeedEnvelopeShard, encodeFeedEnvelopeShard, randomPeerId } from "./feed/veil";
import type { ChannelDirectoryBundle, PostBundle, ProfileBundle } from "./feed/types";

export function App() {
  const [pubkey, setPubkey] = useState("11".repeat(32));
  const [namespace, setNamespace] = useState(7);
  const [channelId, setChannelId] = useState("general");
  const [feedTag, setFeedTag] = useState<string>("");
  const [epoch, setEpoch] = useState<number>(0);
  const [draft, setDraft] = useState("");
  const [model, setModel] = useState(() => loadFeedModel("general") ?? createEmptyFeedModel("general"));
  const fastLaneRef = useRef<InMemoryLaneAdapter | null>(null);
  const clientRef = useRef<VeilClient | null>(null);

  useEffect(() => {
    let disposed = false;
    async function hydrateTag() {
      const now = Math.floor(Date.now() / 1000);
      const nextEpoch = await currentEpoch(now);
      const nextFeedTag = await deriveFeedTagHex(pubkey, namespace);
      if (!disposed) {
        setEpoch(nextEpoch);
        setFeedTag(nextFeedTag);
      }
    }
    void hydrateTag();
    return () => {
      disposed = true;
    };
  }, [namespace, pubkey]);

  useEffect(() => {
    if (!feedTag) {
      return;
    }
    const normalizedChannel = channelId.trim() || "general";
    const loaded = loadFeedModel(normalizedChannel) ?? createEmptyFeedModel(normalizedChannel);
    setModel(loaded);

    clientRef.current?.stop();
    const lane = new InMemoryLaneAdapter();
    const client = new VeilClient(
      lane,
      undefined,
      {
        onShard: (_peer, bytes) => {
          const envelope = decodeFeedEnvelopeShard(bytes);
          if (!envelope) {
            return;
          }
          setModel((prev) => applyFeedEnvelope(prev, envelope));
        },
      },
      { pollIntervalMs: 250 },
    );
    client.subscribe(feedTag);
    client.setForwardPeers([]);
    fastLaneRef.current = lane;
    clientRef.current = client;

    const seedModel = createDemoFeedModel(normalizedChannel);
    const envelopes = feedEnvelopesFromModel(seedModel);
    for (const envelope of envelopes) {
      lane.enqueue(randomPeerId(), encodeFeedEnvelopeShard(envelope, feedTag, namespace));
    }
    void (async () => {
      for (let i = 0; i < envelopes.length; i += 1) {
        await client.tick();
      }
    })();

    return () => {
      client.stop();
      clientRef.current = null;
      fastLaneRef.current = null;
    };
  }, [channelId, feedTag, namespace]);

  useEffect(() => {
    if (model.directoryRoot) {
      saveFeedModel(model);
    }
  }, [model]);

  const directory = model.bundleStore[model.directoryRoot];
  const resolvedDirectory =
    directory && directory.kind === "channel_directory"
      ? directory
      : undefined;

  const profileByPubkey = useMemo(() => {
    const map: Record<string, ProfileBundle> = {};
    if (!resolvedDirectory) {
      return map;
    }
    for (const profileRoot of resolvedDirectory.profileRoots) {
      const profile = model.bundleStore[profileRoot];
      if (profile && profile.kind === "profile") {
        map[profile.authorPubkey] = profile;
      }
    }
    return map;
  }, [model.bundleStore, resolvedDirectory]);

  const timeline = useMemo(() => {
    if (!resolvedDirectory) {
      return [];
    }
    return resolvedDirectory.postRoots
      .map((postRoot) => {
        const post = model.bundleStore[postRoot];
        if (!post || post.kind !== "post") {
          return undefined;
        }
        return { postRoot, post };
      })
      .filter((entry): entry is { postRoot: string; post: PostBundle } => Boolean(entry));
  }, [model.bundleStore, resolvedDirectory]);

  function onPublish() {
    const lane = fastLaneRef.current;
    const client = clientRef.current;
    if (!lane || !client || !feedTag) {
      return;
    }
    setModel((prev) => {
      const next = publishPost(prev, pubkey, draft);
      const diff = diffFeedEnvelopes(prev, next);
      for (const envelope of diff) {
        lane.enqueue(
          randomPeerId(),
          encodeFeedEnvelopeShard(envelope, feedTag, namespace),
        );
      }
      void (async () => {
        for (let i = 0; i < diff.length; i += 1) {
          await client.tick();
        }
      })();
      return prev;
    });
    setDraft("");
  }

  function renderDirectory(dir: ChannelDirectoryBundle | undefined) {
    if (!dir) {
      return <p>Directory bundle missing.</p>;
    }
    return (
      <div>
        <div>title: {dir.title}</div>
        <div>profile_roots: {dir.profileRoots.length}</div>
        <div>post_roots: {dir.postRoots.length}</div>
      </div>
    );
  }

  function nameForPubkey(authorPubkey: string): string {
    return profileByPubkey[authorPubkey]?.displayName ?? `${authorPubkey.slice(0, 10)}...`;
  }

  function shortRoot(root: string): string {
    return `${root.slice(0, 12)}...`;
  }

  async function refreshEpoch() {
    const now = Math.floor(Date.now() / 1000);
    setEpoch(await currentEpoch(now));
  }

  return (
    <main style={{ fontFamily: "ui-monospace, SFMono-Regular, Menlo, monospace", padding: 24 }}>
      <h1>VEIL Feed Demo</h1>
      <p>
        Minimal social feed proof with bundle objects and a channel directory index.
      </p>
      <label>
        Publisher pubkey (32-byte hex):
        <input
          value={pubkey}
          onChange={(e) => setPubkey(e.target.value)}
          style={{ display: "block", width: "100%", marginTop: 8, marginBottom: 12 }}
        />
      </label>
      <label>
        Channel id:
        <input
          value={channelId}
          onChange={(e) => setChannelId(e.target.value)}
          style={{ display: "block", marginTop: 8, marginBottom: 12 }}
        />
      </label>
      <label>
        Namespace (feed scope):
        <input
          type="number"
          value={namespace}
          onChange={(e) => setNamespace(Number(e.target.value))}
          style={{ display: "block", marginTop: 8, marginBottom: 12 }}
        />
      </label>
      <div style={{ marginBottom: 16 }}>
        <button onClick={() => void refreshEpoch()}>Refresh epoch</button>
      </div>

      <section style={{ marginBottom: 20 }}>
        <h2>Channel Directory Bundle</h2>
        <div>epoch: {epoch}</div>
        <div>feed_tag: {feedTag}</div>
        <div>directory_root: {shortRoot(model.directoryRoot)}</div>
        {renderDirectory(resolvedDirectory)}
      </section>

      <section style={{ marginBottom: 20 }}>
        <h2>Compose Post Bundle</h2>
        <textarea
          value={draft}
          onChange={(e) => setDraft(e.target.value)}
          placeholder="Write a post..."
          style={{ width: "100%", minHeight: 70, marginBottom: 8 }}
        />
        <button onClick={onPublish} disabled={draft.trim().length === 0}>
          Publish (updates directory)
        </button>
      </section>

      <section>
        <h2>Timeline (resolved from directory.postRoots)</h2>
        {timeline.map(({ postRoot, post }) => (
          <article
            key={postRoot}
            style={{ border: "1px solid #ccc", borderRadius: 8, padding: 12, marginBottom: 10 }}
          >
            <div>
              <strong>{nameForPubkey(post.authorPubkey)}</strong> · root {shortRoot(postRoot)}
            </div>
            <p>{post.text}</p>
            {post.mediaRoots.length > 0 ? (
              <small>media roots: {post.mediaRoots.map(shortRoot).join(", ")}</small>
            ) : null}
          </article>
        ))}
      </section>
    </main>
  );
}
