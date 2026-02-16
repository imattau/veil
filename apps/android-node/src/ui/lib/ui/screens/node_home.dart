import 'dart:async';

import 'package:flutter/material.dart';

import '../../logic/node_service.dart';
import '../widgets/identity_card.dart';
import '../widgets/lane_status_card.dart';
import '../widgets/node_status_card.dart';
import '../widgets/queue_card.dart';
import '../widgets/service_controls.dart';
import '../widgets/publish_card.dart';
import '../widgets/event_log_card.dart';
import '../widgets/semantic_feed_card.dart';
import '../widgets/policy_card.dart';

class NodeHome extends StatefulWidget {
  const NodeHome({super.key});

  @override
  State<NodeHome> createState() => _NodeHomeState();
}

class _NodeHomeState extends State<NodeHome> {
  final NodeService _service = NodeService();
  Timer? _poller;
  String? _lastShownErrorKey;
  DateTime? _lastShownErrorAt;
  static const Duration _errorToastCooldown = Duration(seconds: 20);

  @override
  void initState() {
    super.initState();
    _service.start();
    _service.connectEvents();
    _poller = Timer.periodic(const Duration(seconds: 5), (_) {
      _service.refresh();
    });
  }

  @override
  void dispose() {
    _poller?.cancel();
    _service.disconnectEvents();
    _service.dispose();
    super.dispose();
  }

  Future<void> _exportIdentity() async {
    final result = await _service.exportIdentity();
    if (result == null) return;
    if (!mounted) return;

    showDialog(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Export Identity'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text('Public Key:'),
            SelectableText(
              result['public_key_hex'],
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            ),
            const SizedBox(height: 12),
            const Text(
              'Secret Key (KEEP PRIVATE!):',
              style: TextStyle(color: Colors.red, fontWeight: FontWeight.bold),
            ),
            SelectableText(
              result['secret_key_hex'],
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  Future<void> _importIdentity() async {
    final controller = TextEditingController();
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Import Identity'),
        content: TextField(
          controller: controller,
          decoration: const InputDecoration(
            labelText: 'Secret Key (Hex)',
            hintText: '64 hex characters',
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(context),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.pop(context, controller.text),
            child: const Text('Import'),
          ),
        ],
      ),
    );

    if (result != null && result.isNotEmpty) {
      await _service.importIdentity(result);
    }
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _service,
      builder: (context, _) {
        final error = _service.state.lastError;
        if (error != null && error.isNotEmpty) {
          final shouldShow = _shouldShowErrorToast(error);
          WidgetsBinding.instance.addPostFrameCallback((_) {
            if (shouldShow && mounted) {
              ScaffoldMessenger.of(context)
                ..hideCurrentSnackBar()
                ..showSnackBar(SnackBar(content: Text(error)));
            }
            _service.clearError();
          });
        }
        return Scaffold(
          appBar: AppBar(
            title: const Text('Veil Node'),
            backgroundColor: const Color(0xFF0B1D26),
            foregroundColor: Colors.white,
          ),
          body: Padding(
            padding: const EdgeInsets.all(20),
            child: ListView(
              children: [
                NodeStatusCard(state: _service.state),
                const SizedBox(height: 16),
                ServiceControls(
                  busy: _service.state.busy,
                  running: _service.state.running,
                  onStart: _service.start,
                  onStop: _service.stop,
                  onRefresh: _service.refresh,
                ),
                const SizedBox(height: 24),
                IdentityCard(
                  identityHex: _service.state.identityHex,
                  onRotate: _service.rotateIdentity,
                  onExport: _exportIdentity,
                  onImport: _importIdentity,
                  busy: _service.state.busy,
                ),
                const SizedBox(height: 16),
                LaneStatusCard(status: _service.state.statusPayload),
                const SizedBox(height: 16),
                QueueCard(status: _service.state.statusPayload),
                const SizedBox(height: 16),
                PublishCard(
                  busy: _service.state.busy,
                  onPublish: (payload) =>
                      _service.publishRaw(payload: payload, namespace: 32),
                ),
                const SizedBox(height: 16),
                PolicyCard(
                  summary: _service.state.policySummary,
                  busy: _service.state.busy,
                  onAction: _service.updatePolicyAction,
                  onExplain: _service.explainPolicy,
                ),
                const SizedBox(height: 16),
                SemanticFeedCard(events: _service.feedEvents),
                const SizedBox(height: 16),
                EventLogCard(events: _service.events),
                const SizedBox(height: 24),
                Text(
                  'Node RPC',
                  style: Theme.of(context).textTheme.titleMedium,
                ),
                const SizedBox(height: 8),
                const Text(
                  'This UI talks directly to the local node HTTP/WS API. '
                  'The SDK is not used.',
                ),
              ],
            ),
          ),
        );
      },
    );
  }

  bool _shouldShowErrorToast(String message) {
    final now = DateTime.now();
    final key = _errorToastKey(message);
    if (_lastShownErrorKey == key &&
        _lastShownErrorAt != null &&
        now.difference(_lastShownErrorAt!) < _errorToastCooldown) {
      return false;
    }
    _lastShownErrorKey = key;
    _lastShownErrorAt = now;
    return true;
  }

  String _errorToastKey(String message) {
    if (message.startsWith('Publish failed')) return 'publish_failed';
    if (message.startsWith('Publish dropped')) return 'publish_dropped';
    if (message.startsWith('Cannot reach node')) return 'cannot_reach_node';
    final normalized = message
        .replaceAll(RegExp(r'\(attempt \d+\)'), '(attempt)')
        .replaceAll(RegExp(r'retry in \d+ms'), 'retry in Xms');
    return normalized;
  }
}
