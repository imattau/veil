import 'package:flutter/material.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';

class PollWidget extends StatelessWidget {
  final NodeEvent event;
  final SocialController controller;

  const PollWidget({super.key, required this.event, required this.controller});

  @override
  Widget build(BuildContext context) {
    if (!event.isPoll) return const SizedBox.shrink();

    final question = event.data['question'] as String? ?? '';
    final options = (event.data['options'] as List?)?.cast<String>() ?? [];
    final root = event.objectRoot;

    // Get votes for this poll
    final votes = controller.nodeService.feedEvents
        .where(
          (e) =>
              e.isFeedBundle &&
              e.data['kind'] == 'poll_vote' &&
              e.data['poll_root'] == root,
        )
        .toList();

    final totalVotes = votes.length;
    final selfPubkey = controller.nodeService.state.identityHex;
    final myVote = votes.firstWhere(
      (v) => v.authorPubkey == selfPubkey,
      orElse: () => const NodeEvent(seq: 0, event: 'unknown', data: {}),
    );
    final hasVoted = myVote.seq != 0;

    return Container(
      margin: const EdgeInsets.symmetric(vertical: 12),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: Colors.white.withOpacity(0.02),
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: Colors.white.withOpacity(0.05)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            question,
            style: const TextStyle(fontWeight: FontWeight.bold, fontSize: 15),
          ),
          const SizedBox(height: 16),
          ...List.generate(options.length, (index) {
            final optionVotes =
                votes.where((v) => v.data['option_index'] == index).length;
            final percent = totalVotes == 0 ? 0.0 : optionVotes / totalVotes;
            final isMyVote = hasVoted && myVote.data['option_index'] == index;

            return Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: InkWell(
                onTap: (root == null || hasVoted)
                    ? null
                    : () async {
                        await controller.nodeService.publishPollVote(
                          pollRoot: root,
                          optionIndex: index,
                          channelId: event.channelId ?? 'general',
                        );
                      },
                child: Stack(
                  children: [
                    Container(
                      height: 36,
                      decoration: BoxDecoration(
                        color: Colors.white.withOpacity(0.05),
                        borderRadius: BorderRadius.circular(8),
                        border: isMyVote
                            ? Border.all(color: VeilTheme.accent, width: 1)
                            : null,
                      ),
                    ),
                    FractionallySizedBox(
                      widthFactor: percent,
                      child: Container(
                        height: 36,
                        decoration: BoxDecoration(
                          color: isMyVote
                              ? VeilTheme.accent.withOpacity(0.4)
                              : VeilTheme.accent.withOpacity(0.2),
                          borderRadius: BorderRadius.circular(8),
                        ),
                      ),
                    ),
                    Container(
                      height: 36,
                      padding: const EdgeInsets.symmetric(horizontal: 12),
                      alignment: Alignment.centerLeft,
                      child: Row(
                        mainAxisAlignment: MainAxisAlignment.spaceBetween,
                        children: [
                          Row(
                            children: [
                              Text(
                                options[index],
                                style: TextStyle(
                                  fontSize: 13,
                                  fontWeight: isMyVote ? FontWeight.bold : null,
                                ),
                              ),
                              if (isMyVote)
                                const Padding(
                                  padding: EdgeInsets.only(left: 8),
                                  child: Icon(Icons.check_circle,
                                      size: 14, color: VeilTheme.accent),
                                ),
                            ],
                          ),
                          if (totalVotes > 0)
                            Text(
                              '${(percent * 100).toInt()}%',
                              style: Theme.of(context).textTheme.labelSmall,
                            ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            );
          }),
          const SizedBox(height: 8),
          Text(
            '$totalVotes votes',
            style: Theme.of(context).textTheme.labelSmall,
          ),
        ],
      ),
    );
  }
}
