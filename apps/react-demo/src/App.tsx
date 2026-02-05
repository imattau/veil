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
import type { PostBundle, ProfileBundle } from "./feed/types";

const BASE_CHANNELS = ["general", "dev", "media"];

function channelHash16(channelId: string): number {
  let hash = 2166136261;
  for (let i = 0; i < channelId.length; i += 1) {
    hash ^= channelId.charCodeAt(i);
    hash = Math.imul(hash, 16777619);
  }
  return hash & 0xffff;
}

export function App() {
  const [pubkey, setPubkey] = useState("11".repeat(32));
  const [namespace, setNamespace] = useState(7);
  const [channelId, setChannelId] = useState("general");
  const [feedTag, setFeedTag] = useState<string>("");
  const [epoch, setEpoch] = useState<number>(0);
  const [draft, setDraft] = useState("");
  const [model, setModel] = useState(
    () => loadFeedModel("general") ?? createEmptyFeedModel("general"),
  );
  const fastLaneRef = useRef<InMemoryLaneAdapter | null>(null);
  const clientRef = useRef<VeilClient | null>(null);
  const normalizedChannel = useMemo(
    () => channelId.trim().toLowerCase() || "general",
    [channelId],
  );
  const effectiveNamespace = useMemo(
    () => (namespace + channelHash16(normalizedChannel)) & 0xffff,
    [namespace, normalizedChannel],
  );
  const channelOptions = useMemo(() => {
    return Array.from(new Set([...BASE_CHANNELS, normalizedChannel]));
  }, [normalizedChannel]);

  const channelDirectoryInfo = useMemo(() => {
    return channelOptions.map((id) => {
      const saved = loadFeedModel(id);
      return {
        id,
        hasIndex: Boolean(saved?.directoryRoot),
        directoryRoot: saved?.directoryRoot ?? "",
      };
    });
  }, [channelOptions]);

  useEffect(() => {
    let disposed = false;
    async function hydrateTag() {
      const now = Math.floor(Date.now() / 1000);
      const nextEpoch = await currentEpoch(now);
      const nextFeedTag = await deriveFeedTagHex(pubkey, effectiveNamespace);
      if (!disposed) {
        setEpoch(nextEpoch);
        setFeedTag(nextFeedTag);
      }
    }
    void hydrateTag();
    return () => {
      disposed = true;
    };
  }, [effectiveNamespace, pubkey]);

  useEffect(() => {
    if (!feedTag) {
      return;
    }
    const loaded =
      loadFeedModel(normalizedChannel) ?? createEmptyFeedModel(normalizedChannel);
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

    if (!loaded.directoryRoot) {
      const seedModel = createDemoFeedModel(normalizedChannel);
      const envelopes = feedEnvelopesFromModel(seedModel);
      for (const envelope of envelopes) {
        lane.enqueue(
          randomPeerId(),
          encodeFeedEnvelopeShard(envelope, feedTag, effectiveNamespace),
        );
      }
      void (async () => {
        for (let i = 0; i < envelopes.length; i += 1) {
          await client.tick();
        }
      })();
    }

    return () => {
      client.stop();
      clientRef.current = null;
      fastLaneRef.current = null;
    };
  }, [effectiveNamespace, feedTag, normalizedChannel]);

  useEffect(() => {
    if (model.directoryRoot) {
      saveFeedModel(model);
    }
  }, [model]);

  const directory = model.bundleStore[model.directoryRoot];
  const resolvedDirectory =
    directory && directory.kind === "channel_directory" ? directory : undefined;

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
      .filter(
        (entry): entry is { postRoot: string; post: PostBundle } => Boolean(entry),
      );
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
          encodeFeedEnvelopeShard(envelope, feedTag, effectiveNamespace),
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

  function shortRoot(root: string): string {
    return `${root.slice(0, 12)}...`;
  }

  function nameForPubkey(authorPubkey: string): string {
    return (
      profileByPubkey[authorPubkey]?.displayName ?? `${authorPubkey.slice(0, 10)}...`
    );
  }

  async function refreshEpoch() {
    const now = Math.floor(Date.now() / 1000);
    setEpoch(await currentEpoch(now));
  }

  return (
    <main className="app-shell">
      <div className="hero-bg" />
      <aside className="sidebar">
        <section className="panel panel-directory-list">
          <h2>Feed Directory</h2>
          <p className="panel-note">
            Quick channel list. Select one to switch timeline context.
          </p>
          <div className="channel-list">
            {channelDirectoryInfo.map((entry) => (
              <button
                key={entry.id}
                className={`channel-item ${entry.id === normalizedChannel ? "active" : ""}`}
                onClick={() => setChannelId(entry.id)}
              >
                <strong>#{entry.id}</strong>
                <span>
                  {entry.hasIndex
                    ? `index ${shortRoot(entry.directoryRoot)}`
                    : "no local index yet"}
                </span>
              </button>
            ))}
          </div>
        </section>

        <section className="panel panel-controls">
          <h2>Feed Setup</h2>
          <p className="panel-note">
            Account and channel settings used to derive the channel discovery key.
          </p>
          <div className="field-grid">
            <label>
              Publisher pubkey
              <input value={pubkey} onChange={(e) => setPubkey(e.target.value)} />
            </label>
            <label>
              Channel id
              <input
                value={channelId}
                onChange={(e) => setChannelId(e.target.value)}
              />
            </label>
            <label>
              Base namespace (advanced)
              <input
                type="number"
                value={namespace}
                onChange={(e) => setNamespace(Number(e.target.value))}
              />
            </label>
          </div>

          <div className="meta-row">
            <div className="meta-pill">
              <span>epoch (time window)</span>
              <strong>{epoch}</strong>
            </div>
            <div className="meta-pill meta-wide">
              <span>channel discovery key (feed tag)</span>
              <strong>{feedTag}</strong>
            </div>
            <div className="meta-pill">
              <span>effective namespace</span>
              <strong>{effectiveNamespace}</strong>
            </div>
            <button className="btn-subtle" onClick={() => void refreshEpoch()}>
              Update time window
            </button>
          </div>
        </section>

        <section className="panel panel-directory">
          <h2>Feed Index</h2>
          <p className="panel-note">
            Points to current profile and post IDs for this channel.
          </p>
          {resolvedDirectory ? (
            <div className="directory-grid">
              <div>
                <span className="dim">title</span>
                <p>{resolvedDirectory.title}</p>
              </div>
              <div>
                <span className="dim">current feed index ID</span>
                <p>{shortRoot(model.directoryRoot)}</p>
              </div>
              <div>
                <span className="dim">profile IDs</span>
                <p>{resolvedDirectory.profileRoots.length}</p>
              </div>
              <div>
                <span className="dim">post IDs</span>
                <p>{resolvedDirectory.postRoots.length}</p>
              </div>
            </div>
          ) : (
            <p>Directory bundle missing.</p>
          )}
        </section>
      </aside>

      <section className="content">
        <section className="panel panel-intro">
          <p className="eyebrow">Shard-native social demo</p>
          <h1>VEIL Feed</h1>
          <p className="lead">
            This demo keeps a lightweight feed index and uses it to load profile and
            post records for the timeline.
          </p>
        </section>

        <section className="panel panel-compose">
          <h2>Create Post</h2>
          <p className="panel-note">
            Creates a post record and updates the feed index to include it.
          </p>
          <textarea
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder="Post to channel..."
          />
          <button
            className="btn-primary"
            onClick={onPublish}
            disabled={draft.trim().length === 0}
          >
            Publish
          </button>
        </section>

        <section className="timeline">
          <h2>Timeline</h2>
          <p className="panel-note">
            Posts shown here are loaded from IDs listed in the current feed index.
          </p>
          {timeline.map(({ postRoot, post }, index) => (
            <article
              className="post-card"
              key={postRoot}
              style={{ animationDelay: `${index * 65}ms` }}
            >
              <header>
                <strong>{nameForPubkey(post.authorPubkey)}</strong>
                <span>{shortRoot(postRoot)}</span>
              </header>
              <div className="card-label">post record</div>
              <p>{post.text}</p>
              {post.mediaRoots.length > 0 ? (
                <small>media: {post.mediaRoots.map(shortRoot).join(", ")}</small>
              ) : null}
            </article>
          ))}
        </section>
      </section>
    </main>
  );
}
