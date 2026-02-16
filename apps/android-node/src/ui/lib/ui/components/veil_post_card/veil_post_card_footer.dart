part of '../veil_post_card.dart';

class _PostFooter extends StatelessWidget {
  final NodeEvent postEvent;
  final String objectRoot;
  final SocialController controller;
  final ListController? listController;
  final bool isDetail;
  final VoidCallback? onCommentTap;

  const _PostFooter({
    required this.postEvent,
    required this.objectRoot,
    required this.controller,
    this.listController,
    required this.isDetail,
    this.onCommentTap,
  });

  @override
  Widget build(BuildContext context) {
    final reactions = controller.getReactions(objectRoot);
    final reposts = controller.getReposts(objectRoot);
    final repostTotal = reposts.length;
    final reposted = controller.hasReposted(objectRoot);
    final comments = controller.getComments(objectRoot);
    final zapTotal = controller.getZapTotal(objectRoot);
    final liked = controller.hasLiked(objectRoot);

    final authorPubkey = postEvent.authorPubkey;
    final lnAddress = authorPubkey != null
        ? controller.nodeService.profiles[authorPubkey]?.lightningAddress
        : null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        ReactionTray(
          objectRoot: objectRoot,
          controller: controller,
          ignoreActions: const ['like'],
        ),
        if (reactions.any((r) => r.reactionAction != 'like'))
          const SizedBox(height: 12),
        Builder(
          builder: (context) {
            final actions = Wrap(
              spacing: 8,
              runSpacing: 4,
              children: [
                _FooterAction(
                  icon: liked ? Icons.favorite : Icons.favorite_border,
                  count: reactions.length,
                  color: liked ? Colors.red : null,
                  onTap: () {
                    HapticFeedback.lightImpact();
                    if (liked) {
                      controller.unlikePost(objectRoot);
                    } else {
                      controller.reactToPost(
                        objectRoot,
                        action: 'like',
                        channelId: postEvent.channelId,
                      );
                    }
                  },
                ),
                _FooterAction(
                  icon: Icons.chat_bubble_outline,
                  count: comments.length,
                  onTap: () {
                    HapticFeedback.lightImpact();
                    if (onCommentTap != null) {
                      onCommentTap!();
                    } else if (!isDetail) {
                      Navigator.push(
                        context,
                        MaterialPageRoute(
                          builder: (context) => PostDetailView(
                            post: postEvent,
                            controller: controller,
                          ),
                        ),
                      );
                    }
                  },
                ),
                _FooterAction(
                  icon: Icons.repeat,
                  count: repostTotal,
                  color: reposted ? VeilTheme.accent : null,
                  onTap: () async {
                    HapticFeedback.lightImpact();
                    final confirm = await showDialog<bool>(
                      context: context,
                      builder: (context) => AlertDialog(
                        title: Text(reposted ? 'Unboost Post?' : 'Boost Post?'),
                        content: Text(
                          reposted
                              ? 'Are you sure you want to remove your boost?'
                              : 'Boosting will share this post with your followers.',
                        ),
                        actions: [
                          TextButton(
                            onPressed: () => Navigator.pop(context, false),
                            child: const Text('Cancel'),
                          ),
                          TextButton(
                            onPressed: () => Navigator.pop(context, true),
                            child: Text(reposted ? 'Unboost' : 'Boost'),
                          ),
                        ],
                      ),
                    );

                    if (confirm == true) {
                      if (reposted) {
                        controller.unrepost(objectRoot);
                      } else {
                        controller.repost(
                          objectRoot,
                          channelId: postEvent.channelId,
                        );
                      }
                    }
                  },
                ),
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
                          controller: controller.zapController,
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

            if (listController == null) {
              return actions;
            }

            return Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(child: actions),
                ListenableBuilder(
                  listenable: listController!,
                  builder: (context, _) {
                    final isBookmarked = listController!.isBookmarked(
                      objectRoot,
                    );
                    return IconButton(
                      onPressed: () {
                        HapticFeedback.lightImpact();
                        listController!.toggleBookmark(objectRoot);
                      },
                      icon: Icon(
                        isBookmarked ? Icons.bookmark : Icons.bookmark_border,
                        size: 18,
                        color: isBookmarked
                            ? VeilTheme.accent
                            : VeilTheme.textSecondary,
                      ),
                    );
                  },
                ),
              ],
            );
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
      borderRadius: BorderRadius.circular(8),
      child: Container(
        constraints: const BoxConstraints(minWidth: 48, minHeight: 48),
        padding: const EdgeInsets.symmetric(horizontal: 4),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 20, color: color ?? VeilTheme.textSecondary),
            if (count > 0) ...[
              const SizedBox(width: 6),
              Text(
                count.toString(),
                style: TextStyle(
                  fontSize: 13,
                  color: color ?? VeilTheme.textSecondary,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}
