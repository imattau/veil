import 'package:flutter/material.dart';

import '../../logic/node_service.dart';
import '../components/empty_state.dart';
import '../theme/veil_theme.dart';

class ExploreView extends StatelessWidget {
  final NodeService service;

  const ExploreView({super.key, required this.service});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: service,
      builder: (context, _) {
        final subscribed = service.state.subscriptions.toSet();
        
        // Optimize: Count posts per channel once
        final counts = <String, int>{};
        for (final event in service.feedEvents) {
          final channel = event.channelId;
          if (channel != null && channel.isNotEmpty) {
            counts[channel] = (counts[channel] ?? 0) + 1;
          }
        }

        final discovered = counts.keys.toSet();
        final channels = <String>{...subscribed, ...discovered}.toList()
          ..sort((a, b) => (counts[b] ?? 0).compareTo(counts[a] ?? 0)); // Sort by popularity

        if (channels.isEmpty) {
          return const EmptyState(
            icon: Icons.tag,
            title: 'No channels yet',
            message:
                'Tap + to add a channel. Channels you post in or discover will show here.',
          );
        }

        return RefreshIndicator(
          onRefresh: service.refresh,
          child: ListView.builder(
            padding: const EdgeInsets.fromLTRB(16, 16, 16, 100),
            itemCount: channels.length + 1,
            itemBuilder: (context, index) {
              if (index == 0) {
                return const Padding(
                  padding: EdgeInsets.only(bottom: 16),
                  child: Text(
                    'Channels',
                    style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
                  ),
                );
              }

              final channel = channels[index - 1];
              final isSubscribed = subscribed.contains(channel);
              final postCount = counts[channel] ?? 0;

              return Card(
                margin: const EdgeInsets.only(bottom: 12),
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: ListTile(
                    leading: Container(
                      width: 48,
                      height: 48,
                      decoration: BoxDecoration(
                        color: VeilTheme.accent.withOpacity(0.1),
                        borderRadius: BorderRadius.circular(12),
                      ),
                      child: const Icon(
                        Icons.tag,
                        color: VeilTheme.accent,
                      ),
                    ),
                    title: Text(
                      '#$channel',
                      style: const TextStyle(
                        fontWeight: FontWeight.bold,
                        fontSize: 16,
                      ),
                    ),
                    subtitle: Text(
                      '$postCount recent events',
                      style: const TextStyle(color: VeilTheme.textSecondary),
                    ),
                    trailing: FilledButton.tonal(
                      onPressed: () async {
                        final ok = isSubscribed
                            ? await service.unsubscribeTag(channel)
                            : await service.subscribeTag(channel);
                        if (!context.mounted || !ok) return;
                        ScaffoldMessenger.of(context).showSnackBar(
                          SnackBar(
                            content: Text(
                              isSubscribed
                                  ? 'Unsubscribed from #$channel'
                                  : 'Subscribed to #$channel',
                            ),
                          ),
                        );
                      },
                      style: FilledButton.styleFrom(
                        backgroundColor: isSubscribed
                            ? Colors.white.withOpacity(0.1)
                            : VeilTheme.accent.withOpacity(0.2),
                        foregroundColor: isSubscribed
                            ? Colors.white
                            : VeilTheme.accent,
                      ),
                      child: Text(isSubscribed ? 'Joined' : 'Join'),
                    ),
                  ),
                ),
              );
            },
          ),
        );
      },
    );
  }
}
