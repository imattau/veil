export type BundleKind = "profile" | "post" | "media" | "channel_directory";

export interface BundleBase {
  version: 1;
  kind: BundleKind;
  channelId: string;
  authorPubkey: string;
  createdAt: number;
}

export interface ProfileBundle extends BundleBase {
  kind: "profile";
  displayName: string;
  bio: string;
  avatarMediaRoot?: string;
}

export interface MediaBundle extends BundleBase {
  kind: "media";
  mimeType: string;
  url: string;
  bytesHint: number;
}

export interface PostBundle extends BundleBase {
  kind: "post";
  text: string;
  mediaRoots: string[];
  replyToRoot?: string;
}

export interface ChannelDirectoryBundle extends BundleBase {
  kind: "channel_directory";
  title: string;
  about: string;
  profileRoots: string[];
  postRoots: string[];
}

export type FeedBundle =
  | ProfileBundle
  | MediaBundle
  | PostBundle
  | ChannelDirectoryBundle;

export interface FeedModel {
  channelId: string;
  bundleStore: Record<string, FeedBundle>;
  directoryRoot: string;
}
