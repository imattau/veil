import 'package:flutter/material.dart';

class QueueCard extends StatelessWidget {
  final Map<String, dynamic> status;

  const QueueCard({super.key, required this.status});

  @override
  Widget build(BuildContext context) {
    final queue = (status['queue'] as Map<String, dynamic>?) ?? {};
    final pending = queue['pending']?.toString() ?? '0';
    final inflight = queue['inflight']?.toString() ?? '0';
    final failed = queue['failed']?.toString() ?? '0';
    final cache = (status['cache'] as Map<String, dynamic>?) ?? {};
    final entries = cache['entries']?.toString() ?? '0';
    final bytes = cache['bytes']?.toString() ?? '0';

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
              'Queue + Cache',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 12),
            _MetricRow(label: 'Pending', value: pending),
            _MetricRow(label: 'Inflight', value: inflight),
            _MetricRow(label: 'Failed', value: failed),
            const Divider(height: 24),
            _MetricRow(label: 'Cache entries', value: entries),
            _MetricRow(label: 'Cache bytes', value: bytes),
          ],
        ),
      ),
    );
  }
}

class _MetricRow extends StatelessWidget {
  final String label;
  final String value;

  const _MetricRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        children: [
          SizedBox(
            width: 140,
            child: Text(
              label,
              style: const TextStyle(fontWeight: FontWeight.w600),
            ),
          ),
          Text(value),
        ],
      ),
    );
  }
}
