import 'dart:async';

import 'package:flutter/material.dart';

import '../../services/node_service.dart';
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

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _service,
      builder: (context, _) {
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
                  onStart: _service.start,
                  onStop: _service.stop,
                  onRefresh: _service.refresh,
                ),
                const SizedBox(height: 24),
                IdentityCard(
                  identityHex: _service.state.identityHex,
                  onRotate: _service.rotateIdentity,
                  busy: _service.state.busy,
                ),
                const SizedBox(height: 16),
                LaneStatusCard(status: _service.state.statusPayload),
                const SizedBox(height: 16),
                QueueCard(status: _service.state.statusPayload),
                const SizedBox(height: 16),
                PublishCard(
                  busy: _service.state.busy,
                  onPublish: (payload) => _service.publishRaw(
                    payload: payload,
                    namespace: 32,
                  ),
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
}
