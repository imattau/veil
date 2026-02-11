import 'package:flutter/material.dart';
import '../theme/veil_theme.dart';

class ExploreView extends StatelessWidget {
  const ExploreView({super.key});

  @override
  Widget build(BuildContext context) {
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        const Text(
          'Channels',
          style: TextStyle(fontSize: 24, fontWeight: FontWeight.bold),
        ),
        const SizedBox(height: 16),
        _ChannelGrid(),
      ],
    );
  }
}

class _ChannelGrid extends StatelessWidget {
  @override
  Widget build(BuildContext context) {
    final channels = [
      {'name': 'general', 'about': 'The main town square of the Veil', 'count': '1.2k'},
      {'name': 'dev', 'about': 'Protocol development and node operations', 'count': '450'},
      {'name': 'news', 'about': 'Global headlines synced via RSS relays', 'count': '890'},
      {'name': 'memes', 'about': 'Decentralized comedy', 'count': '2.1k'},
    ];

    return GridView.builder(
      shrinkWrap: true,
      physics: const NeverScrollableScrollPhysics(),
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 2,
        crossAxisSpacing: 12,
        mainAxisSpacing: 12,
        childAspectRatio: 1.2,
      ),
      itemCount: channels.length,
      itemBuilder: (context, index) {
        final channel = channels[index];
        return Card(
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisAlignment: MainAxisAlignment.center,
              children: [
                Text(
                  '#${channel['name']}',
                  style: const TextStyle(
                    color: VeilTheme.accent,
                    fontWeight: FontWeight.bold,
                    fontSize: 16,
                  ),
                ),
                const SizedBox(height: 4),
                Text(
                  channel['about']!,
                  style: Theme.of(context).textTheme.labelSmall,
                  maxLines: 2,
                  overflow: TextOverflow.ellipsis,
                ),
                const Spacer(),
                Text(
                  '${channel['count']} members',
                  style: const TextStyle(fontSize: 10, color: VeilTheme.textSecondary),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}
