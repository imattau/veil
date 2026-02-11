import 'package:flutter/material.dart';

class LaneStatusCard extends StatelessWidget {
  final Map<String, dynamic> status;

  const LaneStatusCard({super.key, required this.status});

  @override
  Widget build(BuildContext context) {
    final lanes = (status['lanes'] as Map<String, dynamic>?) ?? {};
    final quic = (lanes['quic'] as Map<String, dynamic>?) ?? {};
    final ws = (lanes['websocket'] as Map<String, dynamic>?) ?? {};
    final tor = (lanes['tor'] as Map<String, dynamic>?) ?? {};
    final details = (lanes['details'] as List?)?.cast<Map>() ?? const [];

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
              'Lane Health',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 12),
            _LaneRow(
              label: 'QUIC',
              connected: quic['connected'] == true,
              error: quic['last_error'] as String?,
            ),
            _LaneRow(
              label: 'WebSocket',
              connected: ws['connected'] == true,
              error: ws['last_error'] as String?,
            ),
            _LaneRow(
              label: 'Tor',
              connected: tor['connected'] == true,
              error: tor['last_error'] as String?,
            ),
            if (details.isNotEmpty) ...[
              const SizedBox(height: 12),
              Text(
                'Lane Details',
                style: Theme.of(context).textTheme.labelLarge,
              ),
              const SizedBox(height: 8),
              ...details.map((entry) {
                final role = entry['role'] as String? ?? 'lane';
                final lane = entry['lane'] as String? ?? 'unknown';
                final connected = entry['connected'] == true;
                final error = entry['last_error'] as String?;
                return _LaneRow(
                  label: '$role • $lane',
                  connected: connected,
                  error: error,
                );
              }),
            ],
          ],
        ),
      ),
    );
  }
}

class _LaneRow extends StatelessWidget {
  final String label;
  final bool connected;
  final String? error;

  const _LaneRow({
    required this.label,
    required this.connected,
    required this.error,
  });

  @override
  Widget build(BuildContext context) {
    final color = connected ? const Color(0xFF1B7F5A) : const Color(0xFFB33A3A);
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            width: 10,
            height: 10,
            margin: const EdgeInsets.only(top: 5),
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: const TextStyle(fontWeight: FontWeight.w600),
                ),
                Text(connected ? 'Connected' : 'Disconnected'),
                if (error != null && error!.isNotEmpty)
                  Text(
                    error!,
                    style: TextStyle(color: Theme.of(context).colorScheme.error),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
