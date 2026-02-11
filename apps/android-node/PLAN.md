# Veil Social Implementation Plan

## Phase 1: Foundation & Design System
- [x] **Task 1.1**: Create `lib/ui/theme/veil_theme.dart`.
- [x] **Task 1.2**: Restructure `lib/` for feature-driven architecture.
- [x] **Task 1.3**: Update `NodeState` and `NodeEvent` for rich social data.

## Phase 2: The Social Engine
- [x] **Task 2.1**: Implement `SocialController` for state mapping.
- [x] **Task 2.2**: Implement "Sequence-Based" feed sorting (as reconstructed).
- [x] **Task 2.3**: Implement silent identity generation and onboarding.

## Phase 3: Rich Components
- [x] **Task 3.1**: Build `VeilPostCard` (Text, Media, Boosts, Footer).
- [x] **Task 3.2**: Build `ReactionTray` and `PollWidget`.
- [x] **Task 3.3**: Build `LiveStatusBanner`.

## Phase 4: Views & Navigation
- [x] **Task 4.1**: Implement Tabbed Navigation (Home, Explore, Inbox, Profile).
- [x] **Task 4.2**: Redesign Profile (Identity + Live Status).
- [x] **Task 4.3**: Implement "Network Pulse" connectivity indicator.

## Phase 5: Threading & Discussions
- [x] **Task 5.1**: Expose `replyToRoot` in `NodeEvent`.
- [x] **Task 5.2**: Implement `getComments` in `SocialController`.
- [x] **Task 5.3**: Create `PostDetailView` for conversations.
- [x] **Task 5.4**: Add reply counts and navigation to `VeilPostCard`.
- [x] **Task 5.5**: Implement "Post Reply" capability.

## Phase 6: Secure Messaging UI
- [x] **Task 6.1**: Update `NodeEvent` to handle `payload` events.
- [x] **Task 6.2**: Cache decrypted payloads in `NodeService`.
- [x] **Task 6.3**: Implement `getMessageContent` logic in `MessagingController`.

## Phase 7: Liveness & Engagement
- [x] **Task 7.1**: Implement `ComposerView` (Full-screen post creation).
- [x] **Task 7.2**: Create `StateOverlay` (Syncing / Empty / Error states).
- [x] **Task 7.3**: Implement `FeedShimmer` for startup loading.

## Phase 8: Social Connectivity (Tab Completion)
- [x] **Task 8.1**: Implement `InboxView` (Conversation list).
- [x] **Task 8.2**: Implement `ExploreView` (Channel discovery).
- [x] **Task 8.3**: Implement `ChatDetailView` (DM & Group messaging).

## Phase 9: Security & Persistence
- [x] **Task 9.1**: Implement "Backup Identity" reminder banner.
- [x] **Task 9.2**: Finalize "Edit Profile" (ProfileBundle publication).
- [x] **Task 9.3**: Add Haptic Feedback for actions.

## Phase 10: Visual Refinement
- [x] **Task 10.1**: Apply Glassmorphism to Navigation & AppBars.
- [x] **Task 10.2**: Add "Queue Active" state to `NetworkPulse`.

## Phase 11: Rich Content & Media
- [x] **Task 11.1**: Implement `RichTextView` (Clickable hashtags/mentions/links).
- [x] **Task 11.2**: Build `NestedPostCard` (Visual Quote-Posts/Boosts).
- [ ] **Task 11.3**: Implement `LinkPreviewCard`.
- [ ] **Task 11.4**: Add `MediaGrid` support.
- [ ] **Task 11.5**: Enhance `ComposerView` with social parsing.
- [ ] **Task 11.6**: Implement unit tests for rich content.