part of 'package:veil_android/main.dart';
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
                    const SizedBox(height: 8),
                    SwitchListTile(
                      value: showProtocolDetails,
                      onChanged: onToggleDetails,
                      title: const Text('Show protocol details'),
                      subtitle: const Text('Reveal object_root and lane metadata.'),
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
                      subtitle: const Text('Drop unsigned objects in public namespaces.'),
                    ),
                    TextFormField(
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Clock skew seconds',
                        helperText: 'Adjust for device clock drift (max +/-3600).',
                      ),
                      initialValue: clockSkewSeconds.toString(),
                      onFieldSubmitted: onClockSkewChanged,
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Cache entry limit',
                        helperText: 'Approx shard cache size cap.',
                      ),
                      initialValue: maxCacheEntries.toString(),
                      onFieldSubmitted: onMaxCacheEntriesChanged,
                    ),
                    const SizedBox(height: 12),
                    TextFormField(
                      keyboardType: TextInputType.number,
                      decoration: const InputDecoration(
                        labelText: 'Publish queue limit',
                        helperText: 'Max queued objects pending send.',
                      ),
                      initialValue: maxPublishQueue.toString(),
                      onFieldSubmitted: onMaxPublishQueueChanged,
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

