# VEIL Android UI Requirements (Social Feed App)

This document describes expected UI features for a standard social feed app built over a local VEIL node. It is non-normative; protocol logic lives in the node.

## Core Principles
- UI is a thin client over the local node API.
- The node owns identity, transport lanes, shard cache, and policy.
- The UI only submits payloads and renders decrypted semantic content.

## MVP Feature Set

### 1) Home Feed
- Infinite scroll timeline with pull-to-refresh.
- Post cards with author name, time, content, and media thumbnails.
- Basic reactions (like/boost/reply) if supported by payload schema.
- Offline indicator with queued publish status.

### 2) Compose
- Text post composer with attachments (images/video/files).
- Draft preservation (local).
- Publish queue status (queued/sent/failed).

### 3) Profiles
- Profile view (avatar, name, bio, stats).
- List of posts by profile.
- Follow/unfollow (if supported by schema).

### 4) Channels
- Join/leave channel (tag-based).
- Channel-specific feed.
- Channel search and recent channels.

### 5) Discovery
- Share/scan contact bundles (QR or deep link).
- Add bootstrap nodes by scan or paste.
- Show contact bundle QR for self.

### 6) Notifications (MVP)
- Reply and mention notifications.
- Simple badge counts.

### 7) Settings
- Identity display (public ID, recovery phrase).
- Storage/cache usage.
- Basic app toggles (data saver, media auto-download).

## UX Requirements
- Fast load with skeleton placeholders.
- Clear offline/online status.
- Media viewer with swipe/zoom.
- Feed items must render without blocking UI thread.

## Non-Goals for MVP
- Full moderation tooling in UI.
- Advanced analytics.
- Multi-account switching.

## Node API Expectations (UI-Facing)
- `get_status`: lane health, queue depth, cache size.
- `subscribe(tag)`, `unsubscribe(tag)`.
- `publish(payload)` returning `message_id` and status updates.
- `feed_stream`: event stream of decrypted posts.
- `profile_fetch(pubkey)` and `profile_stream(pubkey)`.
- `contact_bundle_export` and `contact_bundle_import`.
- `add_bootstrap(endpoint)` and `list_bootstrap()`.

## Accessibility
- Large text support.
- Screen reader-friendly labels.
- Minimum contrast for text and interactive controls.

