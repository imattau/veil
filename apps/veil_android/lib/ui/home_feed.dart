import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_blurhash/flutter_blurhash.dart';
import 'package:veil_sdk/veil_sdk.dart';

import '../app_controller.dart';
import '../helpers/strings.dart';
import '../helpers/hashtags.dart';
import '../helpers/scan_helpers.dart';
import '../helpers/media_viewer.dart';
import '../models.dart';
import 'inspect.dart';
import 'widgets.dart';

class HomeFeed extends StatelessWidget {
  final VeilAppController controller;
  final bool showProtocolDetails;
  final VoidCallback onOpenNetwork;
  final VoidCallback onOpenDiscovery;
  final VoidCallback onQuickStart;

  const HomeFeed({
    super.key,
    required this.controller,
    required this.showProtocolDetails,
    required this.onOpenNetwork,
    required this.onOpenDiscovery,
    required this.onQuickStart,
  });

  @override
  Widget build(BuildContext context) {
    final items = controller.visibleFeed;
    return ListView.builder(
      padding: const EdgeInsets.all(16),
      itemCount: items.isEmpty ? 1 : items.length,
      itemBuilder: (context, index) {
        if (items.isEmpty) {
          return _EmptyFeedState(
            controller: controller,
            status: controller.connectionStatus,
            onOpenNetwork: onOpenNetwork,
            onOpenDiscovery: onOpenDiscovery,
            onQuickStart: onQuickStart,
          );
        }
        final entry = items[index];
        return PostCard(
          controller: controller,
          entry: entry,
          showProtocolDetails: showProtocolDetails,
          onTapHashtag: (tag) async {
            await controller.addSubscription(tag);
            if (!context.mounted) return;
            ScaffoldMessenger.of(context).showSnackBar(
              SnackBar(
                content: Text('Joined channel #$tag'),
                behavior: SnackBarBehavior.floating,
                action: SnackBarAction(
                  label: 'Open',
                  onPressed: () {
                    controller.updateSubscription(tag);
                  },
                ),
              ),
            );
          },
        );
      },
    );
  }
}

class _EmptyFeedState extends StatelessWidget {
  final VeilAppController controller;
  final String status;
  final VoidCallback onOpenNetwork;
  final VoidCallback onOpenDiscovery;
  final VoidCallback onQuickStart;

  const _EmptyFeedState({
    required this.controller,
    required this.status,
    required this.onOpenNetwork,
    required this.onOpenDiscovery,
    required this.onQuickStart,
  });

  @override
  Widget build(BuildContext context) {
    final isOffline = status == 'OFFLINE';
    return Container(
      padding: const EdgeInsets.all(32),
      alignment: Alignment.center,
      child: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Icon(
            isOffline ? Icons.cloud_off : Icons.auto_awesome,
            size: 48,
            color: Theme.of(context).colorScheme.primary.withOpacity(0.8),
          ),
          const SizedBox(height: 16),
          Text(
            isOffline ? VeilStrings.feedEmptyOffline : VeilStrings.feedEmptyReady,
            style: Theme.of(context).textTheme.headlineSmall,
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 8),
          Text(
            isOffline
                ? VeilStrings.feedConnectPrompt
                : VeilStrings.feedEmptyPrompt,
            style: Theme.of(context).textTheme.bodyLarge?.copyWith(
                  color: Theme.of(context).textTheme.bodySmall?.color,
                ),
            textAlign: TextAlign.center,
          ),
          const SizedBox(height: 32),
          if (!isOffline)
            SizedBox(
              width: double.infinity,
              child: FilledButton.icon(
                onPressed: onQuickStart,
                style: FilledButton.styleFrom(
                  padding: const EdgeInsets.symmetric(vertical: 16),
                ),
                icon: const Icon(Icons.rocket_launch),
                label: const Text(VeilStrings.quickStart),
              ),
            ),
          const SizedBox(height: 16),
          Wrap(
            spacing: 8,
            alignment: WrapAlignment.center,
            children: [
              TextButton.icon(
                onPressed: () => openScanner(
                  context,
                  onResult: controller.handleScanValue,
                ),
                icon: const Icon(Icons.qr_code_scanner, size: 18),
                label: const Text(VeilStrings.scanCode),
              ),
              TextButton.icon(
                onPressed: onOpenDiscovery,
                icon: const Icon(Icons.explore, size: 18),
                label: const Text(VeilStrings.explore),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class PostCard extends StatelessWidget {
  final VeilAppController controller;
  final FeedEntry entry;
  final bool showProtocolDetails;
  final ValueChanged<String> onTapHashtag;

  const PostCard({
    super.key,
    required this.controller,
    required this.entry,
    required this.showProtocolDetails,
    required this.onTapHashtag,
  });

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<ShardProgress>(
      valueListenable: entry.progressNotifier,
      builder: (context, progress, _) {
        if (entry.isGhost) {
          return Padding(
            padding: const EdgeInsets.only(bottom: 12),
            child: Container(
              height: 160,
              decoration: BoxDecoration(
                gradient: const LinearGradient(
                  colors: [Color(0xFF0F172A), Color(0xFF0B1220)],
                  begin: Alignment.topLeft,
                  end: Alignment.bottomRight,
                ),
                borderRadius: BorderRadius.circular(16),
                border: Border.all(color: const Color(0xFF1F2937)),
                boxShadow: [
                  BoxShadow(
                    color: Colors.black.withOpacity(0.22),
                    blurRadius: 12,
                    offset: const Offset(0, 6),
                  ),
                ],
              ),
              child: Stack(
                children: [
                  BlurPlaceholder(blurHash: entry.blurHash),
                  Align(
                    alignment: Alignment.bottomRight,
                    child: ShardProgressRing(
                      have: progress.have,
                      total: progress.total,
                    ),
                  ),
                ],
              ),
            ),
          );
        }
        return _PostCardAnimated(
          controller: controller,
          entry: entry,
          showProtocolDetails: showProtocolDetails,
          onTapHashtag: onTapHashtag,
        );
      },
    );
  }
}

class _TrustActions extends StatelessWidget {
  final VeilAppController controller;
  final String authorKey;

  const _TrustActions({required this.controller, required this.authorKey});

  @override
  Widget build(BuildContext context) {
    final tier = controller.trustTierFor(authorKey);
    final isTrusted = tier == TrustTier.trusted;
    final isMuted = tier == TrustTier.muted;
    final isBlocked = tier == TrustTier.blocked;

    return Row(
      mainAxisSize: MainAxisSize.min,
      children: [
        IconButton(
          visualDensity: VisualDensity.compact,
          icon: Icon(
            isTrusted ? Icons.favorite : Icons.favorite_border,
            color: isTrusted ? Colors.redAccent : Colors.white70,
            size: 20,
          ),
          onPressed: () {
            if (isTrusted) {
              controller.unfollowUser(authorKey);
            } else {
              controller.followUser(authorKey);
            }
          },
          tooltip: isTrusted ? 'Unfollow' : 'Follow',
        ),
        PopupMenuButton<String>(
          icon: const Icon(Icons.more_vert, size: 20, color: Colors.white70),
          tooltip: 'More options',
          onSelected: (value) {
            switch (value) {
              case 'mute':
                if (isMuted) {
                  controller.unmuteUser(authorKey);
                } else {
                  controller.muteUser(authorKey);
                }
                break;
              case 'block':
                if (isBlocked) {
                  controller.unblockUser(authorKey);
                } else {
                  controller.blockUser(authorKey);
                }
                break;
            }
          },
          itemBuilder: (context) => [
            PopupMenuItem(
              value: 'mute',
              child: Row(
                children: [
                  Icon(
                    isMuted ? Icons.volume_up : Icons.volume_off,
                    size: 18,
                    color: Theme.of(context).iconTheme.color,
                  ),
                  const SizedBox(width: 12),
                  Text(isMuted ? 'Unmute' : 'Mute'),
                ],
              ),
            ),
            PopupMenuItem(
              value: 'block',
              child: Row(
                children: [
                  Icon(
                    isBlocked ? Icons.circle_outlined : Icons.block,
                    size: 18,
                    color: isBlocked
                        ? Theme.of(context).iconTheme.color
                        : Colors.redAccent,
                  ),
                  const SizedBox(width: 12),
                  Text(
                    isBlocked ? 'Unblock' : 'Block',
                    style: TextStyle(
                      color: isBlocked ? null : Colors.redAccent,
                    ),
                  ),
                ],
              ),
            ),
          ],
        ),
      ],
    );
  }
}

class _PostCardAnimated extends StatefulWidget {
  final VeilAppController controller;
  final FeedEntry entry;
  final bool showProtocolDetails;
  final ValueChanged<String> onTapHashtag;

  const _PostCardAnimated({
    required this.controller,
    required this.entry,
    required this.showProtocolDetails,
    required this.onTapHashtag,
  });

  @override
  State<_PostCardAnimated> createState() => _PostCardAnimatedState();
}

class _PostCardAnimatedState extends State<_PostCardAnimated>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 400),
  );

  @override
  void initState() {
    super.initState();
    _controller.forward();
    widget.entry.fadedIn = true;
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final entry = widget.entry;
    final controller = widget.controller;
    return FadeTransition(
      opacity: CurvedAnimation(parent: _controller, curve: Curves.easeOut),
      child: Padding(
        padding: const EdgeInsets.only(bottom: 12),
        child: Container(
          padding: const EdgeInsets.all(16),
          decoration: BoxDecoration(
            gradient: const LinearGradient(
              colors: [Color(0xFF0B1220), Color(0xFF0F172A)],
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
            ),
            borderRadius: BorderRadius.circular(16),
            border: Border.all(color: const Color(0xFF1F2937)),
            boxShadow: [
              BoxShadow(
                color: Colors.black.withOpacity(0.25),
                blurRadius: 14,
                offset: const Offset(0, 8),
              ),
            ],
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const CircleAvatar(
                    radius: 18,
                    backgroundColor: Color(0xFF1E293B),
                    child: Icon(Icons.person, size: 18),
                  ),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      entry.author,
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                  ),
                  Flexible(
                    child: Wrap(
                      spacing: 8,
                      runSpacing: 4,
                      alignment: WrapAlignment.end,
                      children: [
                        if (entry.authorKey.isNotEmpty)
                          _TrustActions(
                            controller: controller,
                            authorKey: entry.authorKey,
                          ),
                        if (entry.reconstructed)
                          const _ReconstructedChip(),
                      ],
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              buildHashtagText(context, entry.body, widget.onTapHashtag),
              if (entry.attachments.isNotEmpty) ...[
                const SizedBox(height: 12),
                SizedBox(
                  height: 120,
                  child: ListView.separated(
                    scrollDirection: Axis.horizontal,
                    itemCount: entry.attachments.length,
                    separatorBuilder: (_, __) => const SizedBox(width: 12),
                    itemBuilder: (context, index) {
                      final attachment = entry.attachments[index];
                      return ClipRRect(
                        borderRadius: BorderRadius.circular(12),
                        child: AspectRatio(
                          aspectRatio: 1,
                          child: attachment.isVideo
                              ? VideoAttachmentPreview(
                                  bytes: attachment.bytes,
                                  title: attachment.name,
                                )
                              : attachment.isImage
                              ? GestureDetector(
                                  onTap: () => openImageViewer(
                                    context,
                                    attachment.bytes,
                                    attachment.name,
                                  ),
                                  child: Image.memory(
                                    attachment.bytes,
                                    fit: BoxFit.cover,
                                  ),
                                )
                              : Container(
                                  color: const Color(0xFF0F172A),
                                  padding: const EdgeInsets.all(12),
                                  child: Column(
                                    mainAxisAlignment: MainAxisAlignment.center,
                                    children: [
                                      const Icon(Icons.insert_drive_file),
                                      const SizedBox(height: 6),
                                      Text(
                                        attachment.name,
                                        maxLines: 2,
                                        overflow: TextOverflow.ellipsis,
                                        textAlign: TextAlign.center,
                                        style: Theme.of(
                                          context,
                                        ).textTheme.bodySmall,
                                      ),
                                      const SizedBox(height: 6),
                                      Text(
                                        '${attachment.chunkCount} chunks',
                                        style: Theme.of(context)
                                            .textTheme
                                            .bodySmall
                                            ?.copyWith(color: Colors.white70),
                                      ),
                                    ],
                                  ),
                                ),
                        ),
                      );
                    },
                  ),
                ),
              ] else if (entry.blurHash != null) ...[
                const SizedBox(height: 12),
                ClipRRect(
                  borderRadius: BorderRadius.circular(12),
                  child: SizedBox(
                    height: 180,
                    width: double.infinity,
                    child: BlurHash(
                      hash: entry.blurHash!,
                      duration: const Duration(milliseconds: 300),
                    ),
                  ),
                ),
              ],
              if (entry.linkPreviews.isNotEmpty) ...[
                const SizedBox(height: 12),
                ...entry.linkPreviews.map(
                  (preview) => LinkPreviewCard(preview: preview),
                ),
              ],
              if (!entry.reconstructed) ...[
                const SizedBox(height: 12),
                Row(
                  children: [
                    ShardProgressRing(
                      have: entry.shardsHave,
                      total: entry.shardsTotal,
                    ),
                    const SizedBox(width: 8),
                    Text(
                      widget.showProtocolDetails
                          ? VeilStrings.collectingShards
                          : VeilStrings.loadingContent,
                      style: Theme.of(
                        context,
                      ).textTheme.bodySmall?.copyWith(color: Colors.white70),
                    ),
                  ],
                ),
                if (entry.requestingMissing && widget.showProtocolDetails) ...[
                  const SizedBox(height: 8),
                  Row(
                    children: [
                      const Icon(
                        Icons.radar,
                        size: 16,
                        color: Color(0xFF38BDF8),
                      ),
                      const SizedBox(width: 6),
                      Text(
                        'Requesting missing shard',
                        style: Theme.of(
                          context,
                        ).textTheme.bodySmall?.copyWith(color: Colors.white70),
                      ),
                    ],
                  ),
                ],
              ],
              if (widget.showProtocolDetails) ...[
                const SizedBox(height: 8),
                Text(
                  'protocol details available in Inspect',
                  style: Theme.of(
                    context,
                  ).textTheme.bodySmall?.copyWith(color: Colors.white70),
                ),
              ],
              Align(
                alignment: Alignment.centerRight,
                child: IconButton(
                  onPressed: () => _openInspect(context, entry),
                  icon: const Icon(Icons.info_outline, size: 20),
                  tooltip: 'Inspect',
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class _ReconstructedChip extends StatelessWidget {
  const _ReconstructedChip();

  @override
  Widget build(BuildContext context) {
    return const Icon(
      Icons.verified,
      size: 18,
      color: Color(0xFF34D399),
      semanticLabel: 'Reconstructed',
    );
  }
}

void _openInspect(BuildContext context, FeedEntry entry) {
  showModalBottomSheet(
    context: context,
    showDragHandle: true,
    backgroundColor: Colors.transparent,
    builder: (context) => InspectSheet(entry: entry),
  );
}
