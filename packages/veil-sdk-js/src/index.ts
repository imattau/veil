export { VeilClient } from "./client";
export type {
  LaneAdapter,
  ForwardingQuotas,
  TierCacheBudgets,
  LaneHealth,
  LaneHealthSnapshot,
  TransportHealthSnapshot,
  VeilClientHooks,
  VeilClientOptions,
} from "./client";
export { MemoryShardCacheStore } from "./storage";
export { AsyncKeyValueShardCacheStore, IndexedDbShardCacheStore } from "./storage";
export type {
  AsyncKeyValueShardCacheOptions,
  AsyncKeyValueStoreLike,
  IndexedDbShardCacheOptions,
  ShardCacheStore,
} from "./storage";
export { InMemoryLaneAdapter, WebSocketLaneAdapter } from "./transports";
export type { WebSocketLaneOptions, WebSocketLike } from "./transports";
export { WebRtcLaneAdapter } from "./webrtc";
export type { WebRtcDataChannelLike, WebRtcLaneOptions } from "./webrtc";
export { MultiLaneAdapter } from "./multi_lane";
export type { MultiLaneSendMode } from "./multi_lane";
export {
  decodeObjectMeta,
  decodeShardMeta,
  validateObjectCbor,
  validateShardCbor,
} from "./codec";
export type { ObjectMeta, ShardMeta } from "./codec";
export {
  buildMediaDescriptorFromChunks,
  decodeAppEnvelope,
  encodeAppEnvelope,
  encodeCanonicalMap,
  encodeFileChunk,
  encodeDirectMessage,
  encodeMediaDescriptor,
  encodeSocialPost,
  encodeProfile,
  extractReferences,
  extractMentions,
  splitIntoFileChunks,
} from "./app_schemas";
export type {
  AppEnvelope,
  FileChunkV1,
  DirectMessageV1,
  MediaDescriptorV1,
  SocialPostV1,
  ProfileV1,
} from "./app_schemas";
export { createAutoFetchPlugin, createThreadContextPlugin } from "./app_plugins";
export type { AutoFetchPluginOptions, RootTagResolver, ThreadContextPluginOptions } from "./app_plugins";
export {
  decodeNamespacePolicy,
  decodeRelayHint,
  decodeRevocation,
  decodeTrustEndorsement,
  encodeNamespacePolicy,
  encodeRelayHint,
  encodeRevocation,
  encodeTrustEndorsement,
} from "./schemas/verification";
export {
  NAMESPACE_APP_BUNDLE,
  NAMESPACE_PRIVATE_VAULT,
  NAMESPACE_PUBLIC_FEED,
  NAMESPACE_RELAY,
  NAMESPACE_RESERVED_MAX,
  NAMESPACE_SYSTEM,
  NAMESPACE_WOT,
} from "./constants";
export type {
  NamespacePolicyV1,
  RelayHintV1,
  RevocationV1,
  TrustEndorsementV1,
} from "./schemas/verification";
export {
  decodePoll,
  decodeReaction,
  decodeVote,
  encodePoll,
  encodeReaction,
  encodeVote,
} from "./schemas/interaction";
export type { PollOptionV1, PollV1, ReactionV1, VoteV1 } from "./schemas/interaction";
export {
  decodeProgressiveImage,
  decodeVideoManifest,
  encodeProgressiveImage,
  encodeVideoManifest,
} from "./schemas/media";
export type { ProgressiveImageV1, VideoManifestV1 } from "./schemas/media";
export {
  createNamespacePolicyPlugin,
  createPollAggregatorPlugin,
  createReactionAggregatorPlugin,
  createRelayHintPlugin,
  createRevocationPlugin,
  createTrustEndorsementPlugin,
  isAppEnvelope,
} from "./social_plugins";
export type {
  NamespacePolicyHandler,
  PollHandler,
  ReactionHandler,
  RelayHintHandler,
  RevocationHandler,
  TrustEndorsementHandler,
  VoteHandler,
} from "./social_plugins";
export {
  decodeBlobManifestV1Bytes,
  decodeDirectoryBundleV1Bytes,
  encodeBlobManifestV1,
  encodeDirectoryBundleV1,
} from "./blob";
export type {
  BlobChunkRefV1,
  BlobManifestV1,
  DirectoryBundleV1,
  DirectoryEntryV1,
} from "./blob";
export { BlobManager } from "./blob_manager";
export type { BlobAssembly } from "./blob_manager";
export {
  buildFileChunks,
  buildMediaDescriptor,
  buildObject,
  buildPostWithAttachments,
  buildSocialPost,
  buildDirectMessage,
  PublishQueue,
} from "./publisher";
export type { PublishBatch, PublishObject } from "./publisher";
export { buildVpsDiscoveryLink } from "./discovery_links";
export { parseDiscoveryInput } from "./discovery_parse";
export {
  configureTagBackend,
  currentEpoch,
  deriveChannelFeedTagHex,
  deriveChannelNamespace,
  deriveChannelRvTagHex,
  deriveFeedTagHex,
  deriveRvTagHex,
  deriveRvTagWindowHex,
  hexToBytes,
  normalizeChannelId,
  initVeilWasm,
  bytesToHex,
} from "./tags";
export { parseVpsConfigJs, toVpsProfileUri, vpsProfileFromDomain } from "./vps";
export type { VpsProfile } from "./vps";
export {
  exportPublicKeyRaw,
  generateEd25519KeyPair,
  hkdfSha256,
  importEd25519PublicKeyRaw,
  exportEd25519PrivateKeyPkcs8,
  importEd25519PrivateKeyPkcs8,
  randomBytes,
  signEd25519,
  verifyEd25519,
} from "./keys";
export {
  AsyncKeyValueIdentityStore,
  MemoryIdentityStore,
  classifyContactBundle,
  contactBundleFromQr,
  contactBundleImportFromQr,
  contactBundleSignedBytes,
  contactBundleToQr,
  decodeContactBundle,
  encodeContactBundle,
  generateIdentity,
  loadOrCreateIdentity,
  mergeContactBundleImport,
  signContactBundle,
  signMessage,
  verifyContactBundle,
  verifyMessage,
} from "./identity";
export type {
  ContactBundle,
  ContactBundleMergeResult,
  ContactBundleImportResult,
  IdentityKeypair,
  IdentityStore,
} from "./identity";
export { defaultWotConfig, LocalWotPolicy, rankFeedItemsByTrust } from "./wot";
export type {
  Endorsement,
  EndorsementRecord,
  PubkeyHex,
  RankableFeedItem,
  TrustScoreExplanation,
  TrustTier,
  WotConfig,
} from "./wot";
