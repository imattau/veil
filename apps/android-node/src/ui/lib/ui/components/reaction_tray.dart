import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';

class ReactionTray extends StatelessWidget {
  final String objectRoot;
  final SocialController controller;
  final List<String> ignoreActions;

  const ReactionTray({
    super.key,
    required this.objectRoot,
    required this.controller,
    this.ignoreActions = const [],
  });

  @override
  Widget build(BuildContext context) {
    final reactions = controller.getReactions(objectRoot);
    if (reactions.isEmpty) return const SizedBox.shrink();

    // Group reactions by action code
    final Map<String, int> counts = {};
    for (var r in reactions) {
      final action = r.reactionAction ?? 'like';
      if (ignoreActions.contains(action)) continue;
      counts[action] = (counts[action] ?? 0) + 1;
    }

    if (counts.isEmpty) return const SizedBox.shrink();

    return Wrap(
      spacing: 8,
      children: counts.entries.map((entry) {
        final action = entry.key;
        final count = entry.value;
        final hasReacted = reactions.any(
          (r) =>
              r.authorPubkey == controller.nodeService.state.identityHex &&
              r.reactionAction == action,
        );

        return GestureDetector(
          onTap: () {
            HapticFeedback.lightImpact();
            if (hasReacted) {
              // Find our specific reaction to this post with this action
              final selfPubkey = controller.nodeService.state.identityHex;
              final myReaction = reactions.firstWhere(
                (r) =>
                    r.authorPubkey == selfPubkey && r.reactionAction == action,
                orElse: () => NodeEvent(seq: 0, event: 'none', data: const {}),
              );
              if (myReaction.event != 'none' && myReaction.objectRoot != null) {
                controller.nodeService.publishDeletion(
                  targetRoots: [myReaction.objectRoot!],
                );
                // Optimistically remove (best effort)
                controller.notifyListeners();
              }
            } else {
              controller.reactToPost(objectRoot, action: action);
            }
          },
          child: Container(
            constraints: const BoxConstraints(minHeight: 40),
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 6),
            decoration: BoxDecoration(
              color: hasReacted
                  ? VeilTheme.accent.withValues(alpha: 0.1)
                  : Colors.white.withValues(alpha: 0.05),
              borderRadius: BorderRadius.circular(20),
              border: Border.all(
                color: hasReacted
                    ? VeilTheme.accent.withValues(alpha: 0.5)
                    : Colors.transparent,
              ),
            ),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(_getEmoji(action), style: const TextStyle(fontSize: 14)),
                const SizedBox(width: 6),
                Text(
                  count.toString(),
                  style: TextStyle(
                    fontSize: 13,
                    color: hasReacted
                        ? VeilTheme.accent
                        : VeilTheme.textSecondary,
                    fontWeight: hasReacted
                        ? FontWeight.bold
                        : FontWeight.normal,
                  ),
                ),
              ],
            ),
          ),
        );
      }).toList(),
    );
  }

  String _getEmoji(String action) {
    switch (action) {
      case 'like':
        return '❤️';
      case 'fire':
        return '🔥';
      case 'rocket':
        return '🚀';
      case 'laugh':
        return '😂';
      default:
        return '👍';
    }
  }
}
