import 'package:flutter/material.dart';
import 'package:veil_sdk/veil_sdk.dart';

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
  late final TextEditingController _wsController;
  late final TextEditingController _wsAddController;
  late final TextEditingController _peerController;
  late final TextEditingController _tagController;
  late final TextEditingController _bleDeviceController;
  late final TextEditingController _bleServiceController;
  late final TextEditingController _bleCharController;
  late final TextEditingController _quicController;
  late final TextEditingController _quicCertController;

  @override
  void initState() {
    super.initState();
    final c = widget.controller;
    _wsController = TextEditingController(text: c.wsUrl);
    _wsAddController = TextEditingController();
    _peerController = TextEditingController(text: c.peerId);
    _tagController = TextEditingController(
      text: c.channelLabel.isNotEmpty ? c.channelLabel : c.tagHex,
    );
    _bleDeviceController = TextEditingController(text: c.bleDeviceId);
    _bleServiceController = TextEditingController(text: c.bleServiceUuid);
    _bleCharController = TextEditingController(text: c.bleCharacteristicUuid);
    _quicController = TextEditingController(text: c.quicEndpointValue);
    _quicCertController = TextEditingController(text: c.quicTrustedCertValue);
  }

  @override
  void dispose() {
    _wsController.dispose();
    _wsAddController.dispose();
    _peerController.dispose();
    _tagController.dispose();
    _bleDeviceController.dispose();
    _bleServiceController.dispose();
    _bleCharController.dispose();
    _quicController.dispose();
    _quicCertController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final theme = Theme.of(context);
    final wsError = _wsController.text.isNotEmpty
        ? controller.wsUrlError
        : null;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Panel(
          title: 'Network Status',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(controller.connected ? 'Connected' : 'Offline'),
              const SizedBox(height: 8),
              Text(
                controller.useLocalRelay
                    ? 'Using internal relay'
                    : 'Using external relay',
              ),
              const SizedBox(height: 8),
              Text('Ghost mode: ${controller.ghostMode ? 'On' : 'Off'}'),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Lane Status',
          child: Column(
            children: [
              _LaneHealthTile(
                title: 'WebSocket Lane',
                icon: controller.connected ? Icons.wifi : Icons.wifi_off,
                label: 'WS',
                enabled: controller.connected,
                snapshot: controller.fastLaneHealth,
              ),
              _LaneHealthTile(
                title: 'Bluetooth Lane',
                icon: Icons.bluetooth,
                label: 'BLE',
                enabled: controller.bleEnabled,
                snapshot: controller.fallbackLaneHealth,
              ),
              _LaneHealthTile(
                title: 'QUIC Lane',
                icon: Icons.bolt,
                label: 'QUIC',
                enabled: controller.quicEndpointValue.isNotEmpty,
                snapshot: controller.quicLaneHealth,
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Connection',
          child: Column(
            children: [
              ListTile(
                leading: Icon(
                  controller.relayReady ? Icons.check_circle : Icons.sync,
                ),
                title: const Text('Local relay'),
                subtitle: Text(
                  controller.relayReady
                      ? 'Internal relay is ready'
                      : 'Starting internal relay...',
                ),
              ),
              const SizedBox(height: 8),
              ExpansionTile(
                title: const Text('Advanced connection'),
                subtitle: Text(
                  'External relay settings',
                  style: theme.textTheme.bodySmall,
                ),
                childrenPadding: const EdgeInsets.only(top: 8),
                children: [
                  InputField(
                    label: 'WebSocket URL',
                    controller: _wsController,
                    onChanged: (value) {
                      controller.setWsUrl(value);
                      setState(() {});
                    },
                    errorText: wsError,
                  ),
                  InputField(
                    label: 'Add WebSocket endpoint',
                    controller: _wsAddController,
                    onChanged: (_) {},
                  ),
                  if (_wsAddController.text.isNotEmpty &&
                      !_wsAddController.text.startsWith('ws://') &&
                      !_wsAddController.text.startsWith('wss://'))
                    Padding(
                      padding: const EdgeInsets.only(bottom: 8),
                      child: Text(
                        'Endpoint must start with ws:// or wss://',
                        style: theme.textTheme.bodySmall?.copyWith(
                          color: Colors.orangeAccent,
                        ),
                      ),
                    ),
                  Align(
                    alignment: Alignment.centerLeft,
                    child: FilledButton.icon(
                      onPressed: () {
                        controller.addWsEndpoint(_wsAddController.text);
                        setState(() {
                          _wsAddController.clear();
                        });
                      },
                      icon: const Icon(Icons.add),
                      label: const Text('Add endpoint'),
                    ),
                  ),
                  if (controller.wsEndpoints.isNotEmpty) ...[
                    const SizedBox(height: 8),
                    Wrap(
                      spacing: 8,
                      runSpacing: 8,
                      children: controller.wsEndpoints
                          .map(
                            (endpoint) => Chip(
                              label: Text(endpoint),
                              onDeleted: () {
                                controller.removeWsEndpoint(endpoint);
                                setState(() {});
                              },
                            ),
                          )
                          .toList(),
                    ),
                  ],
                  InputField(
                    label: 'Peer ID',
                    controller: _peerController,
                    onChanged: (value) {
                      controller.setPeerId(value);
                      setState(() {});
                    },
                  ),
                  InputField(
                    label: 'Channel',
                    controller: _tagController,
                    onChanged: controller.setChannelLabel,
                    errorText: controller.channelError,
                    onScan: () => openScanner(
                      context,
                      onResult: controller.handleScanValue,
                    ),
                  ),
                  const SizedBox(height: 12),
                  Row(
                    children: [
                      Expanded(
                        child: ElevatedButton(
                          onPressed: controller.connected
                              ? null
                              : controller.connect,
                          child: const Text('Connect'),
                        ),
                      ),
                      const SizedBox(width: 12),
                      Expanded(
                        child: OutlinedButton(
                          onPressed: controller.connected
                              ? controller.disconnect
                              : null,
                          child: const Text('Disconnect'),
                        ),
                      ),
                    ],
                  ),
                  const SizedBox(height: 8),
                  OutlinedButton(
                    onPressed: controller.connected
                        ? () =>
                              controller.updateSubscription(_tagController.text)
                        : null,
                    child: const Text('Update Channel'),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Bluetooth Lane',
          child: Column(
            children: [
              SwitchListTile(
                value: controller.bleEnabled,
                onChanged: (value) =>
                    setState(() => controller.setBleEnabled(value)),
                title: const Text('Enable BLE lane'),
                subtitle: const Text('Requires a paired BLE device id.'),
              ),
              InputField(
                label: 'BLE Device ID',
                controller: _bleDeviceController,
                onChanged: controller.setBleDeviceId,
              ),
              InputField(
                label: 'Service UUID',
                controller: _bleServiceController,
                onChanged: controller.setBleServiceUuid,
              ),
              InputField(
                label: 'Characteristic UUID',
                controller: _bleCharController,
                onChanged: controller.setBleCharacteristicUuid,
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'QUIC Lane',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Pinned certs are required for secure QUIC transport.',
                style: theme.textTheme.bodySmall?.copyWith(
                  color: Colors.white70,
                ),
              ),
              const SizedBox(height: 12),
              InputField(
                label: 'QUIC endpoint (quic://host:port)',
                controller: _quicController,
                onChanged: controller.setQuicEndpoint,
              ),
              const SizedBox(height: 12),
              InputField(
                label: 'Trusted cert (hex)',
                controller: _quicCertController,
                onChanged: controller.setQuicTrustedCert,
                minLines: 2,
                maxLines: 3,
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  Icon(
                    controller.quicTrustedCertValue.isEmpty
                        ? Icons.warning_amber_rounded
                        : Icons.verified,
                    size: 16,
                    color: controller.quicTrustedCertValue.isEmpty
                        ? Colors.amber
                        : Colors.greenAccent,
                  ),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      controller.quicTrustedCertValue.isEmpty
                          ? 'No cert pinned'
                          : 'Pinned cert (${controller.quicTrustedCertValue.length ~/ 2} bytes)',
                      style: const TextStyle(
                        color: Colors.white70,
                        fontSize: 12,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () async {
                    await controller.pinQuicCertFromServer();
                    if (!mounted) return;
                    setState(() {
                      _quicCertController.text =
                          controller.quicTrustedCertValue;
                    });
                    final message = controller.quicTrustedCertValue.isEmpty
                        ? 'Failed to pin QUIC cert'
                        : 'Pinned QUIC certificate';
                    ScaffoldMessenger.of(
                      context,
                    ).showSnackBar(SnackBar(content: Text(message)));
                  },
                  icon: const Icon(Icons.shield, size: 18),
                  label: const Text('Pin from server'),
                ),
              ),
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () {
                    controller.setQuicTrustedCert('');
                    setState(() {
                      _quicCertController.text = '';
                    });
                    ScaffoldMessenger.of(context).showSnackBar(
                      const SnackBar(content: Text('Cleared QUIC cert pin')),
                    );
                  },
                  icon: const Icon(Icons.delete_outline, size: 18),
                  label: const Text('Clear pinned cert'),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Recent Activity',
          child: Column(
            children: controller.events
                .take(8)
                .map(
                  (event) => ListTile(
                    dense: true,
                    leading: const Icon(Icons.waves, size: 18),
                    title: Text(event),
                  ),
                )
                .toList(),
          ),
        ),
      ],
    );
  }
}

class _LaneHealthTile extends StatelessWidget {
  final String title;
  final IconData icon;
  final String label;
  final bool enabled;
  final LaneHealthSnapshot? snapshot;

  const _LaneHealthTile({
    required this.title,
    required this.icon,
    required this.label,
    required this.enabled,
    required this.snapshot,
  });

  @override
  Widget build(BuildContext context) {
    final health = snapshot;
    final sendOk = health?.outboundSendOk ?? 0;
    final sendErr = health?.outboundSendErr ?? 0;
    final inbound = health?.inboundReceived ?? 0;
    final dropped = health?.inboundDropped ?? 0;
    final queued = health?.outboundQueued ?? 0;
    final reconnects = health?.reconnectAttempts ?? 0;
    final sendTotal = sendOk + sendErr;
    final okRatio = sendTotal == 0 ? 1.0 : sendOk / sendTotal;

    return ListTile(
      dense: true,
      leading: Icon(icon, size: 18),
      title: Text(title),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(enabled ? 'Healthy' : 'Idle'),
          const SizedBox(height: 4),
          Text(
            'ok ${(okRatio * 100).toStringAsFixed(0)}% · '
            'queued $queued · in $inbound · drop $dropped · retry $reconnects',
            style: Theme.of(
              context,
            ).textTheme.bodySmall?.copyWith(color: Colors.white60),
          ),
        ],
      ),
      trailing: Text(label),
    );
  }
}
