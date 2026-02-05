# Feed Bundle Schema (Demo)

This demo uses app-level bundle objects carried as opaque VEIL shard payloads.

## Bundle kinds

- `profile`
  - `displayName`, `bio`, optional `avatarMediaRoot`
- `post`
  - `text`, optional `mediaRoots[]`, optional `replyToRoot`
- `media`
  - `mimeType`, `url`, `bytesHint`
- `channel_directory`
  - `title`, `about`, `profileRoots[]`, `postRoots[]`

All bundles share:

- `version` (currently `1`)
- `channelId`
- `authorPubkey`
- `createdAt`

## Directory-first feed resolution

Clients resolve timeline content from a channel directory bundle:

1. load `directoryRoot`
2. read `postRoots[]`
3. resolve each root to a `post` bundle
4. resolve referenced profile/media roots

When publishing:

1. create `post` bundle (new root)
2. create updated `channel_directory` bundle (new root)
3. broadcast both bundles
4. move channel head to new directory root

This keeps indexing simple and makes timeline sync deterministic.
