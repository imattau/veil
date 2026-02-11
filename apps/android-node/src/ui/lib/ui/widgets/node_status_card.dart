import 'package:flutter/material.dart';

import '../../logic/models/node_state.dart';

class NodeStatusCard extends StatelessWidget {
  final NodeState state;

  const NodeStatusCard({super.key, required this.state});

  @override
  Widget build(BuildContext context) {
    final status = state.statusPayload;
    final nodeId = status['node_id'] as String?;
    final version = status['version'] as String? ??
        state.healthPayload['version'] as String?;
    final lastUpdated = state.lastUpdated != null
        ? state.lastUpdated!.toLocal().toIso8601String()
        : 'Unknown';

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
              'Node Status',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 12),
            _StatusRow(label: 'Running', value: state.running ? 'Yes' : 'No'),
            _StatusRow(label: 'Node ID', value: nodeId ?? 'Unknown'),
            _StatusRow(label: 'Version', value: version ?? 'Unknown'),
            _StatusRow(label: 'Last update', value: lastUpdated),
            const SizedBox(height: 8),
            const Text(
              'Subscriptions',
              style: TextStyle(fontWeight: FontWeight.w600, fontSize: 13),
            ),
            const SizedBox(height: 4),
            if (state.subscriptions.isEmpty)
              const Text('None', style: TextStyle(color: Colors.grey, fontSize: 13))
            else
              Wrap(
                spacing: 6,
                runSpacing: 0,
                children: state.subscriptions
                    .map((s) => Chip(
                          label: Text(s, style: const TextStyle(fontSize: 11)),
                          visualDensity: VisualDensity.compact,
                          padding: EdgeInsets.zero,
                          materialTapTargetSize: MaterialTapTargetSize.shrinkWrap,
                        ))
                    .toList(),
              ),
            if (state.lastError != null) ...[
              const SizedBox(height: 12),
              Text(
                state.lastError!,
                style: TextStyle(
                  color: Theme.of(context).colorScheme.error,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _StatusRow extends StatelessWidget {
  final String label;
  final String value;

  const _StatusRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          SizedBox(
            width: 110,
            child: Text(
              label,
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
          ),
          Expanded(child: Text(value)),
        ],
      ),
    );
  }
}
