import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../app_controller.dart';
import '../helpers/scan_helpers.dart';
import 'widgets.dart';

class NetworkView extends StatefulWidget {
  final VeilAppController controller;

  const NetworkView({super.key, required this.controller});

  @override
  State<NetworkView> createState() => _NetworkViewState();
}

class _NetworkViewState extends State<NetworkView> {
  late final TextEditingController _wsAddController;

  @override
  void initState() {
    super.initState();
    _wsAddController = TextEditingController();
  }

  @override
  void dispose() {
    _wsAddController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final theme = Theme.of(context);
    final wsEndpoints = controller.wsEndpoints;
    final quicEndpoints = controller.quicEndpoints;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Panel(
          title: 'Bootstrap nodes',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Icon(Icons.qr_code_scanner, size: 18),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Scan a veil://vps profile to import nodes.',
                      style: theme.textTheme.bodySmall,
                    ),
                  ),
                  TextButton(
                    onPressed: () => openScanner(
                      context,
                      onResult: controller.handleScanValue,
                    ),
                    child: const Text('Scan'),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              InputField(
                label: 'Paste profile, domain, or endpoint',
                controller: _wsAddController,
                onChanged: (_) {},
              ),
              const Text(
                'e.g. wss://user:pass@node.com/ws/',
                style: TextStyle(fontSize: 10, color: Colors.white38),
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  Expanded(
                    child: FilledButton.icon(
                      onPressed: () {
                        final raw = _wsAddController.text.trim();
                        if (raw.isEmpty) return;
                        controller.handleScanValue(raw);
                        _wsAddController.clear();
                        setState(() {});
                      },
                      icon: const Icon(Icons.add),
                      label: const Text('Add'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton(
                    onPressed: () async {
                      final data = await Clipboard.getData('text/plain');
                      final value = data?.text?.trim();
                      if (value == null || value.isEmpty) return;
                      _wsAddController.text = value;
                      controller.handleScanValue(value);
                      setState(() {});
                    },
                    child: const Text('Paste'),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              Text(
                'Status: ${controller.connectionStatus}',
                style: theme.textTheme.titleMedium,
              ),
              if (!controller.relayReady)
                Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: Text(
                    'Local relay starting…',
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: Colors.orangeAccent,
                    ),
                  ),
                ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Bootstrap list',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Endpoints (priority order)',
                style: theme.textTheme.labelLarge?.copyWith(
                  color: Colors.white70,
                ),
              ),
              const SizedBox(height: 6),
              if (quicEndpoints.isNotEmpty || wsEndpoints.isNotEmpty) ...[
                Column(
                  children: (() {
                    final rows = <_EndpointRow>[];
                    for (final endpoint in quicEndpoints) {
                      final status = controller.endpointStatusLabel(endpoint);
                      rows.add(
                        _EndpointRow(
                          index: rows.length + 1,
                          label: 'QUIC',
                          value: endpoint,
                          status: status,
                          onCopy: () async {
                            await Clipboard.setData(
                              ClipboardData(text: endpoint),
                            );
                            if (!context.mounted) return;
                            ScaffoldMessenger.of(context).showSnackBar(
                              const SnackBar(
                                content: Text('Endpoint copied'),
                              ),
                            );
                          },
                          onRemove: () {
                            controller.removeQuicEndpoint(endpoint);
                            setState(() {});
                          },
                        ),
                      );
                    }
                    for (final endpoint in wsEndpoints) {
                      final status = controller.endpointStatusLabel(endpoint);
                      rows.add(
                        _EndpointRow(
                          index: rows.length + 1,
                          label: 'WS',
                          value: endpoint,
                          status: status,
                          onCopy: () async {
                            await Clipboard.setData(
                              ClipboardData(text: endpoint),
                            );
                            if (!context.mounted) return;
                            ScaffoldMessenger.of(context).showSnackBar(
                              const SnackBar(
                                content: Text('Endpoint copied'),
                              ),
                            );
                          },
                          onRemove: () {
                            controller.removeWsEndpoint(endpoint);
                            setState(() {});
                          },
                        ),
                      );
                    }
                    return rows;
                  })(),
                ),
              ] else
                Text(
                  'No endpoints yet.',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: Colors.white60,
                  ),
                ),
            ],
          ),
        ),
      ],
    );
  }
}

class _EndpointRow extends StatelessWidget {
  final int index;
  final String label;
  final String value;
  final String status;
  final VoidCallback onCopy;
  final VoidCallback onRemove;

  const _EndpointRow({
    required this.index,
    required this.label,
    required this.value,
    required this.status,
    required this.onCopy,
    required this.onRemove,
  });

  @override
  Widget build(BuildContext context) {
    Color statusColor;
    switch (status) {
      case 'Healthy':
        statusColor = Colors.greenAccent;
        break;
      case 'Unstable':
        statusColor = Colors.orangeAccent;
        break;
      case 'Unreachable':
        statusColor = Colors.redAccent;
        break;
      case 'Pending':
        statusColor = Colors.blueAccent;
        break;
      default:
        statusColor = Colors.grey;
    }
    return ListTile(
      dense: true,
      contentPadding: EdgeInsets.zero,
      leading: Text(
        '#$index',
        style: Theme.of(context).textTheme.bodySmall?.copyWith(
              color: Colors.white60,
            ),
      ),
      title: Text('$label · $value'),
      subtitle: Row(
        children: [
          Icon(Icons.circle, size: 10, color: statusColor),
          const SizedBox(width: 6),
          Text(
            status,
            style: Theme.of(context)
                .textTheme
                .bodySmall
                ?.copyWith(color: Colors.white60),
          ),
        ],
      ),
      trailing: Wrap(
        spacing: 4,
        children: [
          IconButton(
            tooltip: 'Copy',
            icon: const Icon(Icons.copy, size: 18),
            onPressed: onCopy,
          ),
          IconButton(
            tooltip: 'Remove',
            icon: const Icon(Icons.close, size: 18),
            onPressed: onRemove,
          ),
        ],
      ),
    );
  }
}
