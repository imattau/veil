import 'dart:ui';

import 'package:flutter/material.dart';

import '../app_controller.dart';

class SettingsSheet extends StatelessWidget {
  final VeilAppController controller;
  final bool showProtocolDetails;
  final ValueChanged<bool> onToggleDetails;
  final bool ghostMode;
  final ValueChanged<bool> onToggleGhostMode;
  final bool requireSignedPublic;
  final ValueChanged<bool> onToggleRequireSigned;
  final int clockSkewSeconds;
  final ValueChanged<String> onClockSkewChanged;
  final int maxCacheEntries;
  final int maxPublishQueue;
  final ValueChanged<String> onMaxCacheEntriesChanged;
  final ValueChanged<String> onMaxPublishQueueChanged;

  const SettingsSheet({
    super.key,
    required this.controller,
    required this.showProtocolDetails,
    required this.onToggleDetails,
    required this.ghostMode,
    required this.onToggleGhostMode,
    required this.requireSignedPublic,
    required this.onToggleRequireSigned,
    required this.clockSkewSeconds,
    required this.onClockSkewChanged,
    required this.maxCacheEntries,
    required this.maxPublishQueue,
    required this.onMaxCacheEntriesChanged,
    required this.onMaxPublishQueueChanged,
  });

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 16, sigmaY: 16),
          child: Container(
            color: Theme.of(context).colorScheme.surface.withOpacity(0.85),
            child: DraggableScrollableSheet(
              initialChildSize: 0.65,
              minChildSize: 0.4,
              maxChildSize: 0.95,
              expand: false,
              builder: (context, scrollController) {
                return ListView(
                  controller: scrollController,
                  padding: const EdgeInsets.all(16),
                  children: [
                    ExpansionTile(
                      title: const Text('Advanced Settings'),
                      children: [
                        SwitchListTile(
                          value: showProtocolDetails,
                          onChanged: onToggleDetails,
                          title: const Text('Show protocol details'),
                          subtitle: const Text(
                            'Reveal object_root and lane metadata.',
                          ),
                        ),
                        SwitchListTile(
                          value: ghostMode,
                          onChanged: onToggleGhostMode,
                          title: const Text('Ghost mode'),
                          subtitle: const Text('Prefer privacy lanes (preview).'),
                        ),
                        SwitchListTile(
                          value: requireSignedPublic,
                          onChanged: onToggleRequireSigned,
                          title: const Text('Require signed public posts'),
                          subtitle: const Text(
                            'Drop unsigned objects in public namespaces.',
                          ),
                        ),
                        Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 16,
                            vertical: 8,
                          ),
                          child: TextFormField(
                            keyboardType: TextInputType.number,
                            decoration: const InputDecoration(
                              labelText: 'Clock skew seconds',
                              helperText:
                                  'Adjust for device clock drift (max +/-3600).',
                            ),
                            initialValue: clockSkewSeconds.toString(),
                            onFieldSubmitted: onClockSkewChanged,
                          ),
                        ),
                        Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 16,
                            vertical: 8,
                          ),
                          child: TextFormField(
                            keyboardType: TextInputType.number,
                            decoration: const InputDecoration(
                              labelText: 'Cache entry limit',
                              helperText: 'Approx shard cache size cap.',
                            ),
                            initialValue: maxCacheEntries.toString(),
                            onFieldSubmitted: onMaxCacheEntriesChanged,
                          ),
                        ),
                        Padding(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 16,
                            vertical: 8,
                          ),
                          child: TextFormField(
                            keyboardType: TextInputType.number,
                            decoration: const InputDecoration(
                              labelText: 'Publish queue limit',
                              helperText: 'Max queued objects pending send.',
                            ),
                            initialValue: maxPublishQueue.toString(),
                            onFieldSubmitted: onMaxPublishQueueChanged,
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'SDK Status',
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const SizedBox(height: 8),
                    _StatusTile(
                      label: 'Identity created',
                      ok: controller.recoveryPhrase.isNotEmpty,
                      detail: controller.recoveryPhrase.isNotEmpty
                          ? 'Recovery phrase set.'
                          : 'No recovery phrase yet.',
                    ),
                    _StatusTile(
                      label: 'Cache ready',
                      ok: controller.cacheReady,
                      detail: controller.cacheReady
                          ? 'Local shard cache initialized.'
                          : 'Cache not initialized.',
                    ),
                    _StatusTile(
                      label: 'WebSocket endpoints',
                      ok: controller.wsUrl.isNotEmpty ||
                          controller.wsEndpoints.isNotEmpty,
                      detail: controller.wsEndpoints.isNotEmpty
                          ? '${controller.wsEndpoints.length} endpoints configured.'
                          : controller.wsUrl.isNotEmpty
                              ? 'Primary WS endpoint set.'
                              : 'No WS endpoints configured.',
                    ),
                    _StatusTile(
                      label: 'QUIC enabled',
                      ok: controller.isQuicSupported &&
                          (controller.quicEndpoints.isNotEmpty ||
                              controller.quicEndpointValue.isNotEmpty),
                      detail: !controller.isQuicSupported
                          ? 'QUIC not supported on device.'
                          : controller.quicEndpoints.isNotEmpty
                              ? '${controller.quicEndpoints.length} endpoints configured.'
                              : controller.quicEndpointValue.isNotEmpty
                                  ? 'Primary QUIC endpoint set.'
                                  : 'No QUIC endpoints configured.',
                    ),
                    _StatusTile(
                      label: 'Local relay',
                      ok: controller.relayReady,
                      detail: controller.relayReady
                          ? 'Relay running.'
                          : 'Relay not running.',
                    ),
                    _StatusTile(
                      label: 'Connection',
                      ok: controller.connected,
                      detail: controller.connected
                          ? 'Client connected.'
                          : 'Client offline.',
                    ),
                    const SizedBox(height: 16),
                    Text(
                      'Trust & Safety',
                      style: Theme.of(context).textTheme.titleMedium,
                    ),
                    const SizedBox(height: 8),
                    _TrustList(
                      title: 'Following',
                      items: controller.followedUsers,
                      onRemove: controller.unfollowUser,
                    ),
                    _TrustList(
                      title: 'Muted',
                      items: controller.mutedUsers,
                      onRemove: controller.unmuteUser,
                    ),
                    _TrustList(
                      title: 'Blocked',
                      items: controller.blockedUsers,
                      onRemove: controller.unblockUser,
                    ),
                  ],
                );
              },
            ),
          ),
        ),
      ),
    );
  }
}

class _TrustList extends StatelessWidget {
  final String title;
  final List<String> items;
  final ValueChanged<String> onRemove;

  const _TrustList({
    required this.title,
    required this.items,
    required this.onRemove,
  });

  @override
  Widget build(BuildContext context) {
    if (items.isEmpty) {
      return ListTile(
        dense: true,
        title: Text(title),
        subtitle: const Text('No entries yet.'),
      );
    }
    return ExpansionTile(
      title: Text(title),
      children: items
          .map(
            (item) => ListTile(
              dense: true,
              title: Text(item),
              trailing: IconButton(
                icon: const Icon(Icons.close),
                onPressed: () => onRemove(item),
              ),
            ),
          )
          .toList(),
    );
  }
}

class _StatusTile extends StatelessWidget {
  final String label;
  final bool ok;
  final String detail;

  const _StatusTile({
    required this.label,
    required this.ok,
    required this.detail,
  });

  @override
  Widget build(BuildContext context) {
    final color = ok ? const Color(0xFF047857) : const Color(0xFFB91C1C);
    return ListTile(
      dense: true,
      leading: Icon(ok ? Icons.check_circle : Icons.error, color: color),
      title: Text(label),
      subtitle: Text(detail),
    );
  }
}
