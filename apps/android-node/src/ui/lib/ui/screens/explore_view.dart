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
        final discovered = service.feedEvents
            .map((event) => event.channelId)
            .whereType<String>()
            .map((channel) => channel.trim())
            .where((channel) => channel.isNotEmpty)
            .toSet();

        final channels = <String>{...subscribed, ...discovered}.toList()
          ..sort();

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
          child: ListView(
            padding: const EdgeInsets.all(16),
            children: [
              const Text(
                'Channels',
                style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
              ),
              const SizedBox(height: 12),
              ...channels.map((channel) {
                final isSubscribed = subscribed.contains(channel);
                final postCount = service.feedEvents
                    .where((event) => event.channelId == channel)
                    .length;

                return Card(
                  margin: const EdgeInsets.only(bottom: 10),
                  child: ListTile(
                    title: Text(
                      '#$channel',
                      style: const TextStyle(
                        color: VeilTheme.accent,
                        fontWeight: FontWeight.w700,
                      ),
                    ),
                    subtitle: Text(
                      '$postCount recent events',
                      style: const TextStyle(color: VeilTheme.textSecondary),
                    ),
                    trailing: TextButton(
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
                      child: Text(isSubscribed ? 'Joined' : 'Join'),
                    ),
                  ),
                );
              }),
            ],
          ),
        );
      },
    );
  }
}
