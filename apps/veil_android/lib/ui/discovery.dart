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
    final controller = widget.controller;
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Panel(
          title: 'Suggested Feeds',
          child: Column(
            children: controller.suggestedFeeds
                .map(
                  (feed) => ListTile(
                    dense: true,
                    title: Text(feed),
                    subtitle: Text(
                      controller.trustedFeeds.contains(feed)
                          ? 'Trusted by you'
                          : 'Bootstrap recommendation',
                    ),
                    trailing: controller.trustedFeeds.contains(feed)
                        ? const Icon(Icons.verified, color: Color(0xFF34D399))
                        : const Icon(Icons.add_circle_outline),
                    onTap: () => controller.toggleTrustedFeed(feed),
                  ),
                )
                .toList(),
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Suggested Channels',
          child: Column(
            children: controller.suggestedFeeds
                .map(
                  (feed) => ListTile(
                    dense: true,
                    title: Text(feed),
                    subtitle: const Text('Tap to add as a channel'),
                    trailing: const Icon(Icons.add),
                    onTap: () async {
                      await controller.addChannelLabel(feed);
                      if (!context.mounted) return;
                      ScaffoldMessenger.of(context).showSnackBar(
                        SnackBar(
                          content: Text('Added channel $feed'),
                          behavior: SnackBarBehavior.floating,
                        ),
                      );
                    },
                  ),
                )
                .toList(),
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Add Endpoint',
          child: Column(
            children: [
              InputField(
                label: 'Endpoint (wss://.../ws or veil://vps)',
                controller: _peerController,
                onChanged: (_) {},
                onScan: () => openScanner(
                  context,
                  onResult: controller.handleScanValue,
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
                ],
              ),
            ],
          ),
        ),
      ],
    );
  }
}
