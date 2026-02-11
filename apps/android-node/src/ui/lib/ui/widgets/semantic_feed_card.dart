import 'package:flutter/material.dart';

import '../../models/node_event.dart';

class SemanticFeedCard extends StatelessWidget {
  final List<NodeEvent> events;

  const SemanticFeedCard({super.key, required this.events});

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 0,
      color: Colors.white,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Semantic Feed',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 12),
            if (events.isEmpty)
              const Text('No decoded feed bundles yet.')
            else
              ...events.map((event) {
                return Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: _FeedRow(data: event.data),
                );
              }),
          ],
        ),
      ),
    );
  }
}

class _FeedRow extends StatelessWidget {
  final Map<String, dynamic> data;

  const _FeedRow({required this.data});

  @override
  Widget build(BuildContext context) {
    final kind = data['kind'] as String? ?? 'unknown';
    final channel = data['channel_id'] as String? ?? 'unknown';
    final author = data['author_pubkey_hex'] as String? ??
        data['follower_pubkey_hex'] as String? ??
        data['muter_pubkey_hex'] as String? ??
        data['blocker_pubkey_hex'] as String? ??
        'unknown';
    final text = data['text'] as String? ??
        data['action_code'] as String? ??
        data['group_id'] as String? ??
        data['mime_type'] as String? ??
        '';

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          kind,
          style: const TextStyle(fontWeight: FontWeight.w600),
        ),
        const SizedBox(height: 4),
        Text('Channel: $channel'),
        Text('Author: ${_short(author)}'),
        if (text.isNotEmpty) Text(text),
      ],
    );
  }

  String _short(String value) {
    if (value.length <= 12) return value;
    return '${value.substring(0, 6)}…${value.substring(value.length - 4)}';
  }
}
