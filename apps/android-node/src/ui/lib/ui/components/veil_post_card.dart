import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';
import './reaction_tray.dart';
import './rich_text_view.dart';
import './nested_post_card.dart';
import './zap_dialog.dart';
import '../../logic/zap_controller.dart';
import '../screens/post_detail_view.dart';

class VeilPostCard extends StatelessWidget {
  final NodeEvent event;
  final SocialController controller;
  final bool isDetail;

  const VeilPostCard({
    super.key,
    required this.event,
    required this.controller,
    this.isDetail = false,
  });

  @override
  Widget build(BuildContext context) {
    final pubkey = event.authorPubkey ?? 'unknown';
    final selfPubkey = controller.nodeService.state.identityHex;
    final isSelf = selfPubkey != null && selfPubkey == pubkey;
    final displayName = controller.getDisplayName(pubkey);
    final text = event.isRepost ? event.repostComment : event.postText;
    final time = event.createdAt != null
        ? DateTime.fromMillisecondsSinceEpoch(event.createdAt! * 1000)
        : null;
    final root = event.objectRoot;
    final targetRoot = event.isRepost ? event.targetRoot : null;

    return InkWell(
      onTap: isDetail
          ? null
          : () {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) =>
                      PostDetailView(post: event, controller: controller),
                ),
              );
            },
      child: Card(
        margin: const EdgeInsets.only(bottom: 12),
        child: Padding(
          padding: const EdgeInsets.all(16),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  CircleAvatar(
                    backgroundColor: VeilTheme.accent.withOpacity(0.2),
                    backgroundImage: _getAvatarImage(pubkey),
                    child: _getAvatarImage(pubkey) == null
                        ? Text(displayName.substring(0, 1).toUpperCase())
                        : null,
                  ),
                  const SizedBox(width: 12),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          displayName,
                          style: Theme.of(context).textTheme.titleMedium,
                        ),
                        Text(
                          pubkey.length >= 12
                              ? '@${pubkey.substring(0, 12)}...'
                              : '@$pubkey',
                          style: Theme.of(context).textTheme.labelSmall,
                        ),
                      ],
                    ),
                  ),
                  if (!isSelf && pubkey != 'unknown')
                    PopupMenuButton<_AuthorAction>(
                      tooltip: 'Author actions',
                      icon: const Icon(Icons.more_horiz),
                      onSelected: (action) =>
                          _applyAuthorAction(context, action, pubkey),
                      itemBuilder: (context) {
                        final followed = controller.isFollowed(pubkey);
                        final muted = controller.isMuted(pubkey);
                        final blocked = controller.isBlocked(pubkey);
                        return [
                          PopupMenuItem(
                            value: followed
                                ? _AuthorAction.unfollow
                                : _AuthorAction.follow,
                            child: Text(followed ? 'Unfollow' : 'Follow'),
                          ),
                          PopupMenuItem(
                            value: muted
                                ? _AuthorAction.unmute
                                : _AuthorAction.mute,
                            child: Text(muted ? 'Unmute' : 'Mute'),
                          ),
                          PopupMenuItem(
                            value: blocked
                                ? _AuthorAction.unblock
                                : _AuthorAction.block,
                            child: Text(blocked ? 'Unblock' : 'Block'),
                          ),
                        ];
                      },
                    ),
                  if (time != null)
                    Text(
                      _formatTime(time),
                      style: Theme.of(context).textTheme.labelSmall,
                    ),
                ],
              ),
              const SizedBox(height: 12),
              if (text != null && text.isNotEmpty) RichTextView(text: text),
              if (event.mediaRoots.isNotEmpty) ...[
                const SizedBox(height: 12),
                _PostMediaGallery(
                  mediaRoots: event.mediaRoots,
                  controller: controller,
                ),
              ],

              if (targetRoot != null) ...[
                const SizedBox(height: 12),
                NestedPostCard(targetRoot: targetRoot, controller: controller),
              ],

              if (root != null) ...[
                const SizedBox(height: 12),
                ReactionTray(objectRoot: root, controller: controller),
                const SizedBox(height: 16),
                _PostFooter(
                  postEvent: event,
                  objectRoot: root,
                  controller: controller,
                  isDetail: isDetail,
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _applyAuthorAction(
    BuildContext context,
    _AuthorAction action,
    String pubkey,
  ) async {
    switch (action) {
      case _AuthorAction.follow:
        await controller.followUser(pubkey, channelId: event.channelId);
        break;
      case _AuthorAction.unfollow:
        await controller.unfollowUser(pubkey);
        break;
      case _AuthorAction.mute:
        await controller.muteUser(pubkey, channelId: event.channelId);
        break;
      case _AuthorAction.unmute:
        await controller.unmuteUser(pubkey);
        break;
      case _AuthorAction.block:
        await controller.blockUser(pubkey, channelId: event.channelId);
        break;
      case _AuthorAction.unblock:
        await controller.unblockUser(pubkey);
        break;
    }
    if (!context.mounted) return;
    final err = controller.nodeService.state.lastError;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          err ??
              switch (action) {
                _AuthorAction.follow => 'Followed',
                _AuthorAction.unfollow => 'Unfollowed',
                _AuthorAction.mute => 'Muted',
                _AuthorAction.unmute => 'Unmuted',
                _AuthorAction.block => 'Blocked',
                _AuthorAction.unblock => 'Unblocked',
              },
        ),
      ),
    );
  }

  ImageProvider? _getAvatarImage(String pubkey) {
    final profile = controller.nodeService.profiles[pubkey];
    final root = profile?.data['avatar_media_root'] as String?;
    if (root != null && controller.imageCache.containsKey(root)) {
      return MemoryImage(controller.imageCache[root]!);
    }
    return null;
  }

  String _formatTime(DateTime time) {
    final diff = DateTime.now().difference(time);
    if (diff.inMinutes < 60) return '${diff.inMinutes}m';
    if (diff.inHours < 24) return '${diff.inHours}h';
    return '${diff.inDays}d';
  }
}

enum _AuthorAction { follow, unfollow, mute, unmute, block, unblock }

class _PostMediaGallery extends StatelessWidget {
  final List<String> mediaRoots;
  final SocialController controller;

  const _PostMediaGallery({required this.mediaRoots, required this.controller});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: mediaRoots.map((root) {
        final bytes = controller.imageCache[root];
        return Container(
          margin: const EdgeInsets.only(bottom: 8),
          clipBehavior: Clip.antiAlias,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(12),
            color: Colors.white.withOpacity(0.03),
          ),
          child: bytes != null
              ? Image.memory(
                  bytes,
                  fit: BoxFit.cover,
                  width: double.infinity,
                  height: 220,
                  errorBuilder: (_, __, ___) => _mediaUnavailable(),
                )
              : SizedBox(
                  height: 220,
                  child: Center(
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      color: VeilTheme.accent.withOpacity(0.8),
                    ),
                  ),
                ),
        );
      }).toList(),
    );
  }

  Widget _mediaUnavailable() {
    return const SizedBox(
      height: 220,
      child: Center(
        child: Text(
          'Media unavailable',
          style: TextStyle(color: VeilTheme.textSecondary),
        ),
      ),
    );
  }
}

class _PostFooter extends StatelessWidget {
  final NodeEvent postEvent;
  final String objectRoot;
  final SocialController controller;
  final bool isDetail;

  const _PostFooter({
    required this.postEvent,
    required this.objectRoot,
    required this.controller,
    required this.isDetail,
  });

  @override
  Widget build(BuildContext context) {
    final reactions = controller.getReactions(objectRoot);
    final reposts = controller.getReposts(objectRoot);
    final comments = controller.getComments(objectRoot);
    final zapTotal = controller.getZapTotal(objectRoot);
    final liked = controller.hasLiked(objectRoot);

    final authorPubkey = postEvent.authorPubkey;
    final lnAddress = authorPubkey != null
        ? controller.nodeService.profiles[authorPubkey]?.lightningAddress
        : null;

    return Row(
      children: [
        _FooterAction(
          icon: liked ? Icons.favorite : Icons.favorite_border,
          count: reactions.length,
          color: liked ? Colors.red : null,
          onTap: () {
            HapticFeedback.lightImpact();
            controller.reactToPost(
              objectRoot,
              action: 'like',
              channelId: postEvent.channelId,
            );
          },
        ),
        const SizedBox(width: 24),
        _FooterAction(
          icon: Icons.chat_bubble_outline,
          count: comments.length,
          onTap: () {
            HapticFeedback.lightImpact();
            if (!isDetail) {
              Navigator.push(
                context,
                MaterialPageRoute(
                  builder: (context) =>
                      PostDetailView(post: postEvent, controller: controller),
                ),
              );
            }
          },
        ),
        const SizedBox(width: 24),
        _FooterAction(
          icon: Icons.repeat,
          count: reposts.length,
          onTap: () {
            HapticFeedback.lightImpact();
            controller.repost(objectRoot, channelId: postEvent.channelId);
          },
        ),
        const SizedBox(width: 24),
        _FooterAction(
          icon: Icons.bolt,
          count: zapTotal,
          color: zapTotal > 0 ? Colors.amber : null,
          onTap: () {
            HapticFeedback.lightImpact();
            if (lnAddress != null && authorPubkey != null) {
              showDialog(
                context: context,
                builder: (context) => ZapDialog(
                  lnAddress: lnAddress,
                  targetRoot: objectRoot,
                  authorPubkey: authorPubkey,
                  controller: ZapController(controller.nodeService),
                ),
              );
            } else {
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(
                  content: Text('Author has no Lightning Address set'),
                ),
              );
            }
          },
        ),
      ],
    );
  }
}

class _FooterAction extends StatelessWidget {
  final IconData icon;
  final int count;
  final VoidCallback onTap;
  final Color? color;

  const _FooterAction({
    required this.icon,
    required this.count,
    required this.onTap,
    this.color,
  });

  @override
  Widget build(BuildContext context) {
    return InkWell(
      onTap: onTap,
      child: Row(
        children: [
          Icon(icon, size: 18, color: color ?? VeilTheme.textSecondary),
          const SizedBox(width: 4),
          if (count > 0)
            Text(
              count.toString(),
              style: TextStyle(
                fontSize: 12,
                color: color ?? VeilTheme.textSecondary,
              ),
            ),
        ],
      ),
    );
  }
}
