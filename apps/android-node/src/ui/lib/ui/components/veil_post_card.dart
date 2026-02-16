import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../../logic/list_controller.dart';
import '../theme/veil_theme.dart';
import './reaction_tray.dart';
import './rich_text_view.dart';
import './nested_post_card.dart';
import './zap_dialog.dart';
import './link_preview_card.dart';
import '../screens/post_detail_view.dart';

part 'veil_post_card/veil_post_card_media_gallery.dart';
part 'veil_post_card/veil_post_card_footer.dart';

class VeilPostCard extends StatelessWidget {
  final NodeEvent event;
  final SocialController controller;
  final ListController? listController;
  final bool isDetail;
  final VoidCallback? onCommentTap;

  const VeilPostCard({
    super.key,
    required this.event,
    required this.controller,
    this.listController,
    this.isDetail = false,
    this.onCommentTap,
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
    final avatarImage = _getAvatarImage(pubkey);

    String? firstLink;
    if (text != null) {
      final match = RegExp(r'(https?:\/\/[^\s]+)').firstMatch(text);
      if (match != null) {
        firstLink = match.group(0);
      }
    }

    return InkWell(
      key: ValueKey(root ?? 'seq_${event.seq}'),
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
                  Hero(
                    tag: 'avatar_${pubkey}_${root ?? event.seq}',
                    child: CircleAvatar(
                      backgroundColor: VeilTheme.accentSubtle,
                      backgroundImage: avatarImage,
                      child: avatarImage == null
                          ? Text(
                              displayName.isEmpty
                                  ? '?'
                                  : displayName.substring(0, 1).toUpperCase(),
                            )
                          : null,
                    ),
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
              if (text != null && text.isNotEmpty)
                RichTextView(text: text, controller: controller),
              if (event.mediaRoots.isNotEmpty) ...[
                const SizedBox(height: 12),
                _PostMediaGallery(
                  mediaRoots: event.mediaRoots,
                  controller: controller,
                ),
              ] else if (firstLink != null) ...[
                const SizedBox(height: 8),
                LinkPreviewCard(url: firstLink),
              ],

              if (targetRoot != null) ...[
                const SizedBox(height: 12),
                NestedPostCard(targetRoot: targetRoot, controller: controller),
              ],

              if (root != null) ...[
                const SizedBox(height: 16),
                _PostFooter(
                  postEvent: event,
                  objectRoot: root,
                  controller: controller,
                  listController: listController,
                  isDetail: isDetail,
                  onCommentTap: onCommentTap,
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
    final root = profile?.avatarMediaRoot;
    if (root != null && controller.imageCache.containsKey(root)) {
      return MemoryImage(controller.imageCache[root]!);
    }
    return null;
  }

  String _formatTime(DateTime time) {
    final diff = DateTime.now().difference(time);
    if (diff.inSeconds < 60) return 'now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m';
    if (diff.inHours < 24) return '${diff.inHours}h';
    return '${diff.inDays}d';
  }
}

enum _AuthorAction { follow, unfollow, mute, unmute, block, unblock }
