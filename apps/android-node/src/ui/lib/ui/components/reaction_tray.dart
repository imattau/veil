import 'package:flutter/material.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';

class ReactionTray extends StatelessWidget {
  final String objectRoot;
  final SocialController controller;

  const ReactionTray({
    super.key,
    required this.objectRoot,
    required this.controller,
  });

  @override
  Widget build(BuildContext context) {
    final reactions = controller.getReactions(objectRoot);
    if (reactions.isEmpty) return const SizedBox.shrink();

    // Group reactions by action code
    final Map<String, int> counts = {};
    for (var r in reactions) {
      final action = r.reactionAction ?? 'like';
      counts[action] = (counts[action] ?? 0) + 1;
    }

    return Wrap(
      spacing: 8,
      children: counts.entries.map((entry) {
        final action = entry.key;
        final count = entry.value;
        final hasReacted = reactions.any(
          (r) => r.authorPubkey == controller.nodeService.state.identityHex && r.reactionAction == action
        );

        return Container(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 4),
          decoration: BoxDecoration(
            color: hasReacted ? VeilTheme.accent.withOpacity(0.1) : Colors.white.withOpacity(0.05),
            borderRadius: BorderRadius.circular(12),
            border: Border.all(
              color: hasReacted ? VeilTheme.accent.withOpacity(0.5) : Colors.transparent,
            ),
          ),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              Text(_getEmoji(action), style: const TextStyle(fontSize: 12)),
              const SizedBox(width: 4),
              Text(
                count.toString(),
                style: TextStyle(
                  fontSize: 11,
                  color: hasReacted ? VeilTheme.accent : VeilTheme.textSecondary,
                  fontWeight: hasReacted ? FontWeight.bold : FontWeight.normal,
                ),
              ),
            ],
          ),
        );
      }).toList(),
    );
  }

  String _getEmoji(String action) {
    switch (action) {
      case 'like': return '❤️';
      case 'fire': return '🔥';
      case 'rocket': return '🚀';
      case 'laugh': return '😂';
      default: return '👍';
    }
  }
}
