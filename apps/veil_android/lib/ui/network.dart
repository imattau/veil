import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:veil_sdk/veil_sdk.dart';

import '../app_controller.dart';
import '../helpers/strings.dart';
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
  late final TextEditingController _peerController;
  late final TextEditingController _tagController;
  late final TextEditingController _bleDeviceController;
  late final TextEditingController _bleServiceController;
  late final TextEditingController _bleCharController;
  late final TextEditingController _torWsController;
  late final TextEditingController _torHostController;
  late final TextEditingController _torPortController;
  late final TextEditingController _quicController;
  late final TextEditingController _quicCertController;

  @override
  void initState() {
    super.initState();
    final c = widget.controller;
    _wsAddController = TextEditingController();
    _peerController = TextEditingController(text: c.peerId);
    _tagController = TextEditingController(
      text: c.channelLabel.isNotEmpty ? c.channelLabel : c.tagHex,
    );
    _bleDeviceController = TextEditingController(text: c.bleDeviceId);
    _bleServiceController = TextEditingController(text: c.bleServiceUuid);
    _bleCharController = TextEditingController(text: c.bleCharacteristicUuid);
    _torWsController = TextEditingController(text: c.torWsUrlValue);
    _torHostController = TextEditingController(text: c.torSocksHostValue);
    _torPortController = TextEditingController(text: '${c.torSocksPortValue}');
    _quicController = TextEditingController(text: c.quicEndpointValue);
    _quicCertController = TextEditingController(text: c.quicTrustedCertValue);
  }

  @override
  void dispose() {
    _wsAddController.dispose();
    _peerController.dispose();
    _tagController.dispose();
    _bleDeviceController.dispose();
    _bleServiceController.dispose();
    _bleCharController.dispose();
    _torWsController.dispose();
    _torHostController.dispose();
    _torPortController.dispose();
    _quicController.dispose();
    _quicCertController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final theme = Theme.of(context);
    final rawEndpoint = _wsAddController.text.trim();
    final lowerEndpoint = rawEndpoint.toLowerCase();
    final wsEndpoints = controller.wsEndpoints;
    final quicEndpoints = controller.quicEndpoints;
    final selectedQuic = controller.quicEndpointValue;
    final selectedCert =
        selectedQuic.isNotEmpty ? controller.quicCertFor(selectedQuic) ?? '' : '';
    if (_quicController.text != controller.quicEndpointValue) {
      _quicController.text = controller.quicEndpointValue;
    }
    if (_quicCertController.text != selectedCert) {
      _quicCertController.text = selectedCert;
    }
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Panel(
          title: 'Quick connect',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Icon(Icons.qr_code_scanner, size: 18),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      'Scan a veil://vps profile to import endpoints.',
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
                'Connection strength: ${controller.connectionStrengthLabel}',
                style: theme.textTheme.titleMedium,
              ),
              const SizedBox(height: 6),
              Text(
                controller.quicEndpointValue.isNotEmpty
                    ? 'Primary lane: QUIC'
                    : 'Primary lane: WebSocket',
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
              const SizedBox(height: 12),
              Row(
                children: [
                  Expanded(
                    child: FilledButton(
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
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () => _openDiagnostics(context, controller),
                  icon: const Icon(Icons.insights, size: 18),
                  label: const Text('Diagnostics'),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Primary endpoints',
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
                      rows.add(
                        _EndpointRow(
                          index: rows.length + 1,
                          label: endpoint == selectedQuic ? 'QUIC (primary)' : 'QUIC',
                          value: endpoint,
                          onEdit: () {
                            controller.setQuicEndpoint(endpoint);
                            setState(() {});
                          },
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
                      rows.add(
                        _EndpointRow(
                          index: rows.length + 1,
                          label: 'WS',
                          value: endpoint,
                          onEdit: () {
                            _wsAddController.text = endpoint;
                            setState(() {});
                          },
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
              const SizedBox(height: 12),
              const SizedBox(height: 12),
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () async {
                    await controller.pinQuicCertFromServer(
                      selectedQuic.isEmpty ? null : selectedQuic,
                    );
                    if (!mounted) return;
                    setState(() {
                      _quicCertController.text =
                          controller.quicCertFor(selectedQuic) ?? '';
                    });
                    final message = selectedQuic.isEmpty ||
                            (controller.quicCertFor(selectedQuic) ?? '').isEmpty
                        ? 'Failed to pin QUIC cert'
                        : 'Pinned QUIC certificate';
                    ScaffoldMessenger.of(
                      context,
                    ).showSnackBar(SnackBar(content: Text(message)));
                  },
                  icon: const Icon(Icons.shield, size: 16),
                  label: const Text('Pin QUIC cert'),
                ),
              ),
              if (selectedQuic.isNotEmpty && (selectedCert.isEmpty))
                Padding(
                  padding: const EdgeInsets.only(top: 6),
                  child: Text(
                    VeilStrings.quicInstruction,
                    style: theme.textTheme.bodySmall?.copyWith(
                      color: Colors.orangeAccent,
                    ),
                  ),
                ),
              const SizedBox(height: 12),
              InputField(
                label: 'Trusted cert for selected QUIC (hex)',
                controller: _quicCertController,
                onChanged: (value) {
                  if (selectedQuic.isEmpty) {
                    controller.setQuicTrustedCert(value);
                  } else {
                    controller.setQuicCertFor(selectedQuic, value);
                  }
                },
                minLines: 2,
                maxLines: 3,
              ),
              const SizedBox(height: 8),
              Row(
                children: [
                  Icon(
                    selectedCert.isEmpty
                        ? Icons.warning_amber_rounded
                        : Icons.verified,
                    size: 16,
                    color: selectedCert.isEmpty
                        ? Colors.amber
                        : Colors.greenAccent,
                  ),
                  const SizedBox(width: 6),
                  Expanded(
                    child: Text(
                      selectedCert.isEmpty
                          ? 'No cert pinned'
                          : 'Pinned cert (${selectedCert.length ~/ 2} bytes)',
                      style: const TextStyle(
                        color: Colors.white70,
                        fontSize: 12,
                      ),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
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
              const SizedBox(height: 8),
              OutlinedButton(
                onPressed: controller.connected
                    ? () => controller.updateSubscription(_tagController.text)
                    : null,
                child: const Text('Update Channel'),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        ExpansionTile(
          title: const Text('Advanced lanes'),
          subtitle: Text(
            'BLE and Tor settings',
            style: theme.textTheme.bodySmall,
          ),
          childrenPadding: const EdgeInsets.only(top: 8),
          children: [
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
              title: 'Tor Lane',
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SwitchListTile(
                    value: controller.torEnabled,
                    onChanged: (value) =>
                        setState(() => controller.setTorEnabled(value)),
                    title: const Text('Enable Tor lane'),
                    subtitle:
                        const Text('Requires Orbot running on this device.'),
                  ),
                  InputField(
                    label: 'Tor WebSocket URL (wss://.../ws)',
                    controller: _torWsController,
                    onChanged: controller.setTorWsUrl,
                  ),
                  InputField(
                    label: 'Tor SOCKS host',
                    controller: _torHostController,
                    onChanged: controller.setTorSocksHost,
                  ),
                  InputField(
                    label: 'Tor SOCKS port',
                    controller: _torPortController,
                    onChanged: controller.setTorSocksPort,
                  ),
                ],
              ),
            ),
          ],
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

class _LaneStatusNote extends StatelessWidget {
  final VeilAppController controller;

  const _LaneStatusNote({required this.controller});

  @override
  Widget build(BuildContext context) {
    final quic = controller.quicLaneHealth;
    if (controller.quicEndpointValue.isEmpty) {
      return const SizedBox.shrink();
    }
    if (!controller.isQuicSupported) {
      return Text(
        'QUIC not supported on this device',
        style: Theme.of(context)
            .textTheme
            .bodySmall
            ?.copyWith(color: Colors.redAccent),
      );
    }
    if (quic == null) {
      return Text(
        'QUIC pending. Pin the certificate to enable.',
        style: Theme.of(context)
            .textTheme
            .bodySmall
            ?.copyWith(color: Colors.orangeAccent),
      );
    }
    final ok = quic.outboundSendOk;
    final err = quic.outboundSendErr;
    String status = 'Stable';
    if (ok == 0 && err > 0) {
      status = 'Unreachable';
    } else if (err > ok) {
      status = 'Unstable';
    } else if (quic.outboundQueued > 5) {
      status = 'Congested';
    }
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Text(
        'QUIC status: $status',
        style: Theme.of(context)
            .textTheme
            .bodySmall
            ?.copyWith(color: Colors.white70),
      ),
    );
  }
}

class _LaneHealthTile extends StatelessWidget {
  final String title;
  final IconData icon;
  final String label;
  final bool enabled;
  final LaneHealthSnapshot? snapshot;
  final String? lastError;

  const _LaneHealthTile({
    required this.title,
    required this.icon,
    required this.label,
    required this.enabled,
    required this.snapshot,
    this.lastError,
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

    Color statusColor = Colors.grey;
    String statusText = 'Disabled';

    if (enabled) {
      if (sendErr > 0 && sendOk == 0) {
        statusColor = Colors.redAccent;
        statusText = 'Failing';
      } else if (okRatio < 0.8) {
        statusColor = Colors.orangeAccent;
        statusText = 'Degraded';
      } else if (sendOk > 0) {
        statusColor = Colors.greenAccent;
        statusText = 'Active';
      } else {
        statusColor = Colors.blueAccent;
        statusText = 'Standby';
      }
    }

    return ListTile(
      dense: true,
      leading: Icon(icon, size: 20, color: statusColor),
      title: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Flexible(
            child: Text(
              title,
              overflow: TextOverflow.ellipsis,
            ),
          ),
          const SizedBox(width: 8),
          Container(
            padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 2),
            decoration: BoxDecoration(
              color: statusColor.withOpacity(0.1),
              borderRadius: BorderRadius.circular(4),
              border: Border.all(color: statusColor.withOpacity(0.3)),
            ),
            child: Text(
              statusText.toUpperCase(),
              style: TextStyle(
                color: statusColor,
                fontSize: 9,
                fontWeight: FontWeight.bold,
              ),
            ),
          ),
        ],
      ),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SizedBox(height: 4),
          Text(
            'Success Rate: ${(okRatio * 100).toStringAsFixed(0)}% · '
            'Inbound: $inbound · Retries: $reconnects',
            style: Theme.of(
              context,
            ).textTheme.bodySmall?.copyWith(color: Colors.white60),
            overflow: TextOverflow.ellipsis,
            maxLines: 2,
          ),
          if (queued > 0 || dropped > 0)
            Text(
              'Queued: $queued · Dropped: $dropped',
              style: Theme.of(
                context,
              ).textTheme.bodySmall?.copyWith(color: Colors.orangeAccent.withOpacity(0.8)),
              overflow: TextOverflow.ellipsis,
              maxLines: 1,
            ),
          if (lastError != null && lastError!.isNotEmpty)
            InkWell(
              onTap: () {
                Clipboard.setData(ClipboardData(text: lastError!));
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Error copied')),
                );
              },
              child: Padding(
                padding: const EdgeInsets.only(top: 4),
                child: Text(
                  'Error: $lastError',
                  style: Theme.of(
                    context,
                  ).textTheme.bodySmall?.copyWith(color: Colors.redAccent),
                  overflow: TextOverflow.ellipsis,
                  maxLines: 4,
                ),
              ),
            ),
        ],
      ),
      trailing: Text(
        label,
        style: Theme.of(context).textTheme.bodySmall?.copyWith(color: Colors.white38),
      ),
    );
  }
}

class _EndpointRow extends StatelessWidget {
  final int index;
  final String label;
  final String value;
  final VoidCallback onEdit;
  final VoidCallback onCopy;
  final VoidCallback onRemove;

  const _EndpointRow({
    required this.index,
    required this.label,
    required this.value,
    required this.onEdit,
    required this.onCopy,
    required this.onRemove,
  });

  @override
  Widget build(BuildContext context) {
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
      trailing: Wrap(
        spacing: 4,
        children: [
          IconButton(
            tooltip: 'Edit',
            icon: const Icon(Icons.edit, size: 18),
            onPressed: onEdit,
          ),
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

void _openDiagnostics(BuildContext context, VeilAppController controller) {
  showModalBottomSheet(
    context: context,
    showDragHandle: true,
    backgroundColor: Colors.transparent,
    builder: (context) {
      return Container(
        padding: const EdgeInsets.all(16),
        decoration: const BoxDecoration(
          color: Color(0xFF0B1220),
          borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
        ),
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Panel(
                title: 'Lane diagnostics',
                child: Column(
                  children: [
                    _LaneStatusNote(controller: controller),
                    _LaneHealthTile(
                      title: 'QUIC Lane',
                      icon: Icons.bolt,
                      label: 'QUIC',
                      enabled: controller.quicEndpointValue.isNotEmpty &&
                          controller.isQuicSupported,
                      snapshot: controller.quicLaneHealth,
                    ),
                                      _LaneHealthTile(
                                        title: 'WebSocket Lane',
                                        icon: controller.connected ? Icons.wifi : Icons.wifi_off,
                                        label: 'WS',
                                        enabled: controller.connected,
                                        snapshot: controller.fastLaneHealth,
                                        lastError: controller.wsLastError,
                                      ),                    _LaneHealthTile(
                      title: 'Tor Lane',
                      icon: Icons.shield,
                      label: 'Tor',
                      enabled: controller.torEnabled,
                      snapshot: controller.torLaneHealth,
                    ),
                    _LaneHealthTile(
                      title: 'Bluetooth Lane',
                      icon: Icons.bluetooth,
                      label: 'BLE',
                      enabled: controller.bleEnabled,
                      snapshot: controller.bleLaneHealth,
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      );
    },
  );
}
