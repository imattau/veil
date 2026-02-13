import 'package:flutter/material.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';
import 'rich_text_view.dart';

class NestedPostCard extends StatelessWidget {
  final String targetRoot;
  final SocialController controller;

  const NestedPostCard({
    super.key,
    required this.targetRoot,
    required this.controller,
  });

  @override
  Widget build(BuildContext context) {
    // Find the original post in the feed
    final originalPost = controller.nodeService.feedEvents.firstWhere(
      (e) => e.objectRoot == targetRoot,
      orElse: () => const NodeEvent(seq: 0, event: 'unknown', data: {}),
    );

    if (originalPost.seq == 0) {
      return Container(
        padding: const EdgeInsets.all(12),
        decoration: BoxDecoration(
          color: Colors.white.withOpacity(0.02),
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: Colors.white.withOpacity(0.05)),
        ),
        child: const Text(
          'Post not found or still syncing...',
          style: TextStyle(color: VeilTheme.textSecondary, fontSize: 12),
        ),
      );
    }

    final pubkey = originalPost.authorPubkey ?? 'unknown';
    final displayName = controller.getDisplayName(pubkey);

    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: Colors.white.withOpacity(0.02),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: Colors.white.withOpacity(0.05)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              CircleAvatar(
                radius: 10,
                backgroundColor: VeilTheme.accent.withOpacity(0.2),
                child: Text(
                  displayName.substring(0, 1).toUpperCase(),
                  style: const TextStyle(fontSize: 8),
                ),
              ),
              const SizedBox(width: 8),
              Text(
                displayName,
                style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 12),
              ),
              const SizedBox(width: 4),
              Text(
                pubkey.length >= 8 ? '@${pubkey.substring(0, 8)}' : '@$pubkey',
                style: const TextStyle(color: VeilTheme.textSecondary, fontSize: 10),
              ),
            ],
          ),
          const SizedBox(height: 8),
          RichTextView(
            text: originalPost.postText ?? '',
            controller: controller,
            style: const TextStyle(fontSize: 13),
          ),
        ],
      ),
    );
  }
}
