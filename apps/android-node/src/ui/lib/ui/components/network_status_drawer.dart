import 'package:flutter/material.dart';

import '../../logic/node_service.dart';
import '../theme/veil_theme.dart';

class NetworkStatusDrawer extends StatelessWidget {
  final NodeService service;

  const NetworkStatusDrawer({super.key, required this.service});

  @override
  Widget build(BuildContext context) {
    return Drawer(
      width: 360,
      child: SafeArea(
        child: ListenableBuilder(
          listenable: service,
          builder: (context, _) {
            final state = service.state;
            final status = state.statusPayload;
            final lanes = (status['lanes'] as Map?) ?? const {};
            final details =
                (lanes['details'] as List?)?.cast<Map>() ?? const [];
            final connectedLaneCount = details
                .where((detail) => detail['connected'] == true)
                .length;

            final quicConnected = details.any((d) => d['transport'] == 'quic' && d['connected'] == true);
            final wsConnected = details.any((d) => d['transport'] == 'websocket' && d['connected'] == true);
            final torConnected = details.any((d) => d['transport'] == 'tor' && d['connected'] == true);

            final queue = (status['queue'] as Map?) ?? const {};
            final cache = (status['cache'] as Map?) ?? const {};

            var outboundQueued = 0;
            var outboundOk = 0;
            var outboundErr = 0;
            var inboundReceived = 0;
            var inboundDropped = 0;
            var reconnectAttempts = 0;
            for (final detail in details) {
              final stats = (detail['stats'] as Map?) ?? const {};
              outboundQueued +=
                  (stats['outbound_queued'] as num?)?.toInt() ?? 0;
              outboundOk += (stats['outbound_send_ok'] as num?)?.toInt() ?? 0;
              outboundErr += (stats['outbound_send_err'] as num?)?.toInt() ?? 0;
              inboundReceived +=
                  (stats['inbound_received'] as num?)?.toInt() ?? 0;
              inboundDropped +=
                  (stats['inbound_dropped'] as num?)?.toInt() ?? 0;
              reconnectAttempts +=
                  (stats['reconnect_attempts'] as num?)?.toInt() ?? 0;
            }

            return Column(
              children: [
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 14, 8, 8),
                  child: Row(
                    children: [
                      const Expanded(
                        child: Text(
                          'Network Details',
                          style: TextStyle(
                            fontSize: 18,
                            fontWeight: FontWeight.w700,
                          ),
                        ),
                      ),
                      IconButton(
                        onPressed: service.refresh,
                        icon: const Icon(Icons.refresh),
                        tooltip: 'Refresh',
                      ),
                      IconButton(
                        onPressed: () => Navigator.of(context).maybePop(),
                        icon: const Icon(Icons.close),
                        tooltip: 'Close',
                      ),
                    ],
                  ),
                ),
                const Divider(height: 1),
                Expanded(
                  child: ListView(
                    padding: const EdgeInsets.all(16),
                    children: [
                      _SectionCard(
                        title: 'Connection',
                        rows: [
                          _MetricRowData(
                            'Node running',
                            state.running ? 'Yes' : 'No',
                          ),
                          _MetricRowData(
                            'Connected lanes',
                            '$connectedLaneCount / ${details.length}',
                          ),
                          _MetricRowData(
                            'QUIC',
                            quicConnected ? 'Connected' : 'Disconnected',
                          ),
                          _MetricRowData(
                            'WebSocket',
                            wsConnected ? 'Connected' : 'Disconnected',
                          ),
                          _MetricRowData(
                            'Tor',
                            torConnected ? 'Connected' : 'Disconnected',
                          ),
                          _MetricRowData(
                            'Subscriptions',
                            '${state.subscriptions.length}',
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      _SectionCard(
                        title: 'Queue + Cache',
                        rows: [
                          _MetricRowData(
                            'Pending',
                            '${(queue['pending'] as num?)?.toInt() ?? 0}',
                          ),
                          _MetricRowData(
                            'Inflight',
                            '${(queue['inflight'] as num?)?.toInt() ?? 0}',
                          ),
                          _MetricRowData(
                            'Failed',
                            '${(queue['failed'] as num?)?.toInt() ?? 0}',
                          ),
                          _MetricRowData(
                            'Cache entries',
                            '${(cache['entries'] as num?)?.toInt() ?? 0}',
                          ),
                          _MetricRowData(
                            'Cache bytes',
                            _formatBytes(
                              (cache['bytes'] as num?)?.toInt() ?? 0,
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 12),
                      _SectionCard(
                        title: 'Traffic',
                        subtitle: 'Aggregated from lane runtime counters',
                        rows: [
                          _MetricRowData(
                            'Inbound received',
                            '$inboundReceived',
                          ),
                          _MetricRowData('Inbound dropped', '$inboundDropped'),
                          _MetricRowData('Outbound queued', '$outboundQueued'),
                          _MetricRowData('Outbound sent', '$outboundOk'),
                          _MetricRowData('Outbound errors', '$outboundErr'),
                          _MetricRowData(
                            'Reconnect attempts',
                            '$reconnectAttempts',
                          ),
                        ],
                      ),
                      if (state.lastError != null) ...[
                        const SizedBox(height: 12),
                        Container(
                          padding: const EdgeInsets.all(12),
                          decoration: BoxDecoration(
                            color: Colors.red.withOpacity(0.08),
                            borderRadius: BorderRadius.circular(10),
                            border: Border.all(
                              color: Colors.red.withOpacity(0.35),
                            ),
                          ),
                          child: Text(
                            state.lastError!,
                            style: const TextStyle(color: Colors.redAccent),
                          ),
                        ),
                      ],
                    ],
                  ),
                ),
              ],
            );
          },
        ),
      ),
    );
  }

  String _formatBytes(int value) {
    const units = ['B', 'KB', 'MB', 'GB'];
    double size = value.toDouble();
    var idx = 0;
    while (size >= 1024 && idx < units.length - 1) {
      size /= 1024;
      idx++;
    }
    final shown = idx == 0 ? size.toStringAsFixed(0) : size.toStringAsFixed(1);
    return '$shown ${units[idx]}';
  }
}

class _MetricRowData {
  final String label;
  final String value;

  const _MetricRowData(this.label, this.value);
}

class _SectionCard extends StatelessWidget {
  final String title;
  final String? subtitle;
  final List<_MetricRowData> rows;

  const _SectionCard({required this.title, this.subtitle, required this.rows});

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: VeilTheme.surface,
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: Colors.white10),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title,
            style: const TextStyle(fontWeight: FontWeight.w700, fontSize: 14),
          ),
          if (subtitle != null) ...[
            const SizedBox(height: 2),
            Text(
              subtitle!,
              style: const TextStyle(
                color: VeilTheme.textSecondary,
                fontSize: 12,
              ),
            ),
          ],
          const SizedBox(height: 10),
          ...rows.map(
            (row) => Padding(
              padding: const EdgeInsets.only(bottom: 6),
              child: Row(
                children: [
                  Expanded(
                    child: Text(
                      row.label,
                      style: const TextStyle(
                        color: VeilTheme.textSecondary,
                        fontSize: 13,
                      ),
                    ),
                  ),
                  Text(
                    row.value,
                    style: const TextStyle(fontWeight: FontWeight.w600),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}
