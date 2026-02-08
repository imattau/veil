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
