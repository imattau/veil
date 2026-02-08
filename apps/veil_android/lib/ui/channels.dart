import 'package:flutter/material.dart';

import '../app_controller.dart';
import '../helpers/strings.dart';
import 'widgets.dart';

class ChannelsView extends StatefulWidget {
  final VeilAppController controller;

  const ChannelsView({super.key, required this.controller});

  @override
  State<ChannelsView> createState() => _ChannelsViewState();
}

class _ChannelsViewState extends State<ChannelsView> {
  final _addController = TextEditingController();

  @override
  void dispose() {
    _addController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    final channels = controller.channels;
    return SingleChildScrollView(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Panel(
            title: VeilStrings.navChannels,
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Manage subscriptions and choose the default channel used by Compose.',
                  style: Theme.of(context)
                      .textTheme
                      .bodySmall
                      ?.copyWith(color: Colors.white70),
                ),
                const SizedBox(height: 16),
                Row(
                  children: [
                    Expanded(
                      child: InputField(
                        label: 'Channel name',
                        controller: _addController,
                      ),
                    ),
                    const SizedBox(width: 8),
                    Padding(
                      padding: const EdgeInsets.only(bottom: 12),
                      child: IconButton.filled(
                        onPressed: () async {
                          final value = _addController.text.trim();
                          if (value.isEmpty) return;
                          await controller.addChannelLabel(value);
                          _addController.clear();
                          setState(() {});
                        },
                        icon: const Icon(Icons.add),
                        tooltip: 'Add',
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 8),
                if (channels.isEmpty)
                  Text(
                    'No channels yet.',
                    style: Theme.of(context)
                        .textTheme
                        .bodySmall
                        ?.copyWith(color: Colors.white70),
                  )
                else
                  ...channels.map(
                    (channel) => _ChannelTile(
                      channel: channel,
                      onMakeDefault: () async {
                        await controller.setDefaultChannel(channel.label);
                        setState(() {});
                      },
                      onRemove: () {
                        controller.removeChannelLabel(channel.label);
                        setState(() {});
                      },
                    ),
                  ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _ChannelTile extends StatelessWidget {
  final ChannelInfo channel;
  final VoidCallback onRemove;
  final VoidCallback onMakeDefault;

  const _ChannelTile({
    required this.channel,
    required this.onRemove,
    required this.onMakeDefault,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.only(bottom: 10),
      color: Theme.of(context).colorScheme.surfaceContainerHighest,
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
        child: Row(
          children: [
            Icon(
              channel.isDefault ? Icons.star : Icons.tag,
              color: channel.isDefault ? Colors.amber : Colors.white70,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    channel.label.startsWith('tag:')
                        ? 'Custom tag'
                        : '#${channel.label}',
                    style: Theme.of(context).textTheme.titleSmall,
                  ),
                  const SizedBox(height: 4),
                  Text(
                    channel.tagHex.isEmpty ? 'pending…' : channel.tagHex,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: Theme.of(context)
                        .textTheme
                        .bodySmall
                        ?.copyWith(color: Colors.white60),
                  ),
                ],
              ),
            ),
            const SizedBox(width: 8),
            if (!channel.isDefault)
              IconButton(
                tooltip: 'Set as Default',
                onPressed: onMakeDefault,
                icon: const Icon(Icons.star_border),
              ),
            IconButton(
              tooltip: 'Unsubscribe',
              onPressed: onRemove,
              icon: const Icon(Icons.close),
            ),
          ],
        ),
      ),
    );
  }
}
