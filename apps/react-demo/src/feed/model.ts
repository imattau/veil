import type {
  ChannelDirectoryBundle,
  FeedBundle,
  FeedModel,
  MediaBundle,
  PostBundle,
  ProfileBundle,
} from "./types";

export interface FeedEnvelope {
  root: string;
  bundle: FeedBundle;
  directoryRoot?: string;
}

function randomHex(bytes: number): string {
  const out = new Uint8Array(bytes);
  const cryptoObj = globalThis.crypto;
  if (cryptoObj?.getRandomValues) {
    cryptoObj.getRandomValues(out);
  } else {
    for (let i = 0; i < out.length; i += 1) {
      out[i] = Math.floor(Math.random() * 256);
    }
  }
  return [...out].map((b) => b.toString(16).padStart(2, "0")).join("");
}

function nextRoot(): string {
  return randomHex(32);
}

function insertBundle(
  store: Record<string, FeedBundle>,
  bundle: FeedBundle,
): string {
  const root = nextRoot();
  store[root] = bundle;
  return root;
}

function profileBundle(
  channelId: string,
  authorPubkey: string,
  displayName: string,
  bio: string,
  avatarMediaRoot?: string,
): ProfileBundle {
  return {
    version: 1,
    kind: "profile",
    channelId,
    authorPubkey,
    createdAt: Date.now(),
    displayName,
    bio,
    avatarMediaRoot,
  };
}

function mediaBundle(
  channelId: string,
  authorPubkey: string,
  url: string,
  mimeType = "image/png",
): MediaBundle {
  return {
    version: 1,
    kind: "media",
    channelId,
    authorPubkey,
    createdAt: Date.now(),
    mimeType,
    url,
    bytesHint: 64_000,
  };
}

function postBundle(
  channelId: string,
  authorPubkey: string,
  text: string,
  mediaRoots: string[] = [],
): PostBundle {
  return {
    version: 1,
    kind: "post",
    channelId,
    authorPubkey,
    createdAt: Date.now(),
    text,
    mediaRoots,
  };
}

function directoryBundle(
  channelId: string,
  authorPubkey: string,
  profileRoots: string[],
  postRoots: string[],
): ChannelDirectoryBundle {
  return {
    version: 1,
    kind: "channel_directory",
    channelId,
    authorPubkey,
    createdAt: Date.now(),
    title: `#${channelId}`,
    about: "Channel directory bundle for feed resolution.",
    profileRoots,
    postRoots,
  };
}

export function createDemoFeedModel(channelId: string): FeedModel {
  const store: Record<string, FeedBundle> = {};
  const alice = "aa".repeat(32);
  const bob = "bb".repeat(32);

  const mediaRoot = insertBundle(
    store,
    mediaBundle(channelId, alice, "https://picsum.photos/seed/veil-demo/320/180"),
  );
  const aliceProfile = insertBundle(
    store,
    profileBundle(channelId, alice, "Alice", "Edge relay operator", mediaRoot),
  );
  const bobProfile = insertBundle(
    store,
    profileBundle(channelId, bob, "Bob", "Ships mobile client updates"),
  );
  const postA = insertBundle(
    store,
    postBundle(channelId, alice, "Bootstrapped a new lane pair. Coverage looks good."),
  );
  const postB = insertBundle(
    store,
    postBundle(channelId, bob, "Bundle directory model feels clean for timeline sync.", [mediaRoot]),
  );

  const directoryRoot = insertBundle(
    store,
    directoryBundle(channelId, alice, [aliceProfile, bobProfile], [postB, postA]),
  );

  return {
    channelId,
    bundleStore: store,
    directoryRoot,
  };
}

export function createEmptyFeedModel(channelId: string): FeedModel {
  return {
    channelId,
    bundleStore: {},
    directoryRoot: "",
  };
}

export function publishPost(
  model: FeedModel,
  authorPubkey: string,
  text: string,
): FeedModel {
  const trimmed = text.trim();
  if (!trimmed) {
    return model;
  }
  const store = { ...model.bundleStore };
  const currentDirectory = store[model.directoryRoot];
  if (!currentDirectory || currentDirectory.kind !== "channel_directory") {
    return model;
  }

  const postRoot = insertBundle(
    store,
    postBundle(model.channelId, authorPubkey, trimmed),
  );
  const nextDirectoryRoot = insertBundle(
    store,
    directoryBundle(
      model.channelId,
      authorPubkey,
      currentDirectory.profileRoots,
      [postRoot, ...currentDirectory.postRoots].slice(0, 48),
    ),
  );
  return {
    ...model,
    bundleStore: store,
    directoryRoot: nextDirectoryRoot,
  };
}

export function applyFeedEnvelope(
  model: FeedModel,
  envelope: FeedEnvelope,
): FeedModel {
  if (envelope.bundle.channelId !== model.channelId) {
    return model;
  }
  const bundleStore = {
    ...model.bundleStore,
    [envelope.root]: envelope.bundle,
  };
  const directoryRoot =
    envelope.directoryRoot ?? (
      envelope.bundle.kind === "channel_directory" ? envelope.root : model.directoryRoot
    );
  return {
    ...model,
    bundleStore,
    directoryRoot,
  };
}

export function feedEnvelopesFromModel(model: FeedModel): FeedEnvelope[] {
  const out: FeedEnvelope[] = [];
  for (const [root, bundle] of Object.entries(model.bundleStore)) {
    out.push({
      root,
      bundle,
      directoryRoot:
        bundle.kind === "channel_directory" && root === model.directoryRoot
          ? model.directoryRoot
          : undefined,
    });
  }
  return out;
}

export function diffFeedEnvelopes(
  previous: FeedModel,
  next: FeedModel,
): FeedEnvelope[] {
  const out: FeedEnvelope[] = [];
  for (const [root, bundle] of Object.entries(next.bundleStore)) {
    if (previous.bundleStore[root]) {
      continue;
    }
    out.push({
      root,
      bundle,
      directoryRoot:
        bundle.kind === "channel_directory" && root === next.directoryRoot
          ? next.directoryRoot
          : undefined,
    });
  }
  return out;
}
