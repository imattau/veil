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
  late final TextEditingController _tagController;
  String? _contactBundle;
  bool _contactLoading = false;

  @override
  void initState() {
    super.initState();
    _tagController = TextEditingController();
    _refreshContactBundle();
  }

  @override
  void dispose() {
    _tagController.dispose();
    super.dispose();
  }

  Future<void> _refreshContactBundle() async {
    setState(() {
      _contactLoading = true;
    });
    final value = await widget.controller.buildContactBundleString();
    if (!mounted) return;
    setState(() {
      _contactBundle = value;
      _contactLoading = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final controller = widget.controller;
    final rawTag = _tagController.text.trim();
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        Panel(
          title: 'Share Contact',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Share your contact bundle as a QR code payload.',
                style: theme.textTheme.bodySmall?.copyWith(color: Colors.white60),
              ),
              const SizedBox(height: 12),
              if (_contactLoading)
                const Center(child: CircularProgressIndicator())
              else if (_contactBundle == null || _contactBundle!.isEmpty)
                Text(
                  'Contact bundle unavailable. Try again.',
                  style: theme.textTheme.bodySmall?.copyWith(color: Colors.orangeAccent),
                )
              else
                SelectableText(
                  _contactBundle!,
                  style: theme.textTheme.bodySmall?.copyWith(color: Colors.white70),
                ),
              const SizedBox(height: 12),
              Row(
                children: [
                  Expanded(
                    child: ElevatedButton(
                      onPressed: (_contactBundle == null || _contactBundle!.isEmpty)
                          ? null
                          : () async {
                              await Clipboard.setData(
                                ClipboardData(text: _contactBundle!),
                              );
                              if (!context.mounted) return;
                              ScaffoldMessenger.of(context).showSnackBar(
                                const SnackBar(
                                  content: Text('Contact bundle copied'),
                                  behavior: SnackBarBehavior.floating,
                                ),
                              );
                            },
                      child: const Text('Copy Bundle'),
                    ),
                  ),
                  const SizedBox(width: 8),
                  OutlinedButton(
                    onPressed: _refreshContactBundle,
                    child: const Text('Refresh'),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
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
