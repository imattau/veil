import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../helpers/scan_helpers.dart';

import '../app_controller.dart';
import 'widgets.dart';

class DiscoveryView extends StatefulWidget {
  final VeilAppController controller;

  const DiscoveryView({super.key, required this.controller});

  @override
  State<DiscoveryView> createState() => _DiscoveryViewState();
}

class _DiscoveryViewState extends State<DiscoveryView> {
  late final TextEditingController _peerController;
  late final TextEditingController _tagController;

  @override
  void initState() {
    super.initState();
    _peerController = TextEditingController();
    _tagController = TextEditingController();
  }

  @override
  void dispose() {
    _peerController.dispose();
    _tagController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final controller = widget.controller;
    final rawEndpoint = _peerController.text.trim();
    final lowerEndpoint = rawEndpoint.toLowerCase();
    final rawTag = _tagController.text.trim();
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Panel(
          title: 'Suggested Communities',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Join communities to start receiving specific topic feeds.',
                style: theme.textTheme.bodySmall?.copyWith(color: Colors.white60),
              ),
              const SizedBox(height: 12),
              ...controller.suggestedFeeds.map(
                (feed) => ListTile(
                  dense: true,
                  contentPadding: EdgeInsets.zero,
                  title: Text('#$feed'),
                  subtitle: Text(
                    controller.extraTags.contains(controller.tagHexFor(feed))
                        ? 'Joined'
                        : 'Tap to join',
                  ),
                  trailing: controller.extraTags.contains(controller.tagHexFor(feed))
                      ? const Icon(Icons.check_circle, color: Color(0xFF34D399))
                      : const Icon(Icons.add_circle_outline),
                  onTap: () async {
                    if (controller.extraTags.contains(controller.tagHexFor(feed))) {
                      controller.removeChannelLabel(feed);
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text('Left #$feed'),
                          behavior: SnackBarBehavior.floating,
                        ),
                      );
                    } else {
                      await controller.addChannelLabel(feed);
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text('Joined #$feed'),
                          behavior: SnackBarBehavior.floating,
                        ),
                      );
                    }
                    setState(() {});
                  },
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Add Endpoint',
          child: Column(
            children: [
              InputField(
                label: 'Endpoint (domain, wss://.../ws, or veil://vps)',
                controller: _peerController,
                onChanged: (_) {},
                onScan: () => openScanner(
                  context,
                  onResult: controller.handleScanValue,
                ),
              ),
              if (rawEndpoint.isNotEmpty &&
                  !lowerEndpoint.startsWith('ws://') &&
                  !lowerEndpoint.startsWith('wss://') &&
                  !lowerEndpoint.startsWith('veil://') &&
                  !lowerEndpoint.startsWith('veil:vps:') &&
                  !lowerEndpoint.startsWith('vps:') &&
                  !lowerEndpoint.startsWith('quic://'))
                Padding(
                  padding: const EdgeInsets.only(top: 6, bottom: 6),
                  child: Text(
                    'Endpoint should start with veil://, ws://, wss://, or quic://',
                    style: Theme.of(context)
                        .textTheme
                        .bodySmall
                        ?.copyWith(color: Colors.orangeAccent),
                  ),
                ),
              Row(
                children: [
                  Expanded(
                    child: ElevatedButton(
                      onPressed: () {
                        controller.handleScanValue(_peerController.text);
                        _peerController.clear();
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Endpoint added'),
                            behavior: SnackBarBehavior.floating,
                          ),
                        );
                      },
                      child: const Text('Add Endpoint'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton(
                    onPressed: () async {
                      final data = await Clipboard.getData('text/plain');
                      final value = data?.text?.trim();
                      if (value == null || value.isEmpty) return;
                      _peerController.text = value;
                      controller.handleScanValue(value);
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(
                          content: Text('Endpoint pasted'),
                          behavior: SnackBarBehavior.floating,
                        ),
                      );
                    },
                    child: const Text('Paste'),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Subscribe to Channel',
          child: Column(
            children: [
              InputField(
                label: 'Channel name (or tag:HEX)',
                controller: _tagController,
                onChanged: (_) {},
                onScan: () => openScanner(
                  context,
                  onResult: controller.handleScanValue,
                ),
              ),
              if (rawTag.isNotEmpty &&
                  !(rawTag.toLowerCase().startsWith('tag:')) &&
                  !RegExp(r'^[0-9a-fA-F]{64}$').hasMatch(rawTag) &&
                  !RegExp(r'^[a-zA-Z0-9\\- _]+$').hasMatch(rawTag))
                Padding(
                  padding: const EdgeInsets.only(top: 6, bottom: 6),
                  child: Text(
                    'Use a short name or tag:HEX',
                    style: Theme.of(context)
                        .textTheme
                        .bodySmall
                        ?.copyWith(color: Colors.orangeAccent),
                  ),
                ),
              Row(
                children: [
                  Expanded(
                    child: ElevatedButton(
                      onPressed: () async {
                        await controller.addChannelLabel(_tagController.text);
                        _tagController.clear();
                        if (!context.mounted) return;
                        ScaffoldMessenger.of(context).showSnackBar(
                          const SnackBar(
                            content: Text('Channel added'),
                            behavior: SnackBarBehavior.floating,
                          ),
                        );
                      },
                      child: const Text('Add Channel'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton(
                    onPressed: () async {
                      final data = await Clipboard.getData('text/plain');
                      final value = data?.text?.trim();
                      if (value == null || value.isEmpty) return;
                      _tagController.text = value;
                      await controller.addChannelLabel(value);
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        const SnackBar(
                          content: Text('Channel pasted'),
                          behavior: SnackBarBehavior.floating,
                        ),
                      );
                    },
                    child: const Text('Paste'),
                  ),
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
