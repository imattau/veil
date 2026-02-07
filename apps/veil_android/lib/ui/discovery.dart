import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

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
          title: 'Add Peer',
          child: Column(
            children: [
              InputField(
                label: 'Peer address (ws:// or quic://)',
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
                      onPressed: () =>
                          controller.addForwardPeer(_peerController.text),
                      child: const Text('Add Peer'),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              ...controller.forwardPeers.map(
                (peer) => ListTile(
                  dense: true,
                  leading: const Icon(Icons.router, size: 18),
                  title: Text(peer),
                  trailing: SizedBox(
                    width: 48,
                    height: 48,
                    child: IconButton(
                      icon: const Icon(Icons.close),
                      onPressed: () => controller.removeForwardPeer(peer),
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
        const SizedBox(height: 16),
        Panel(
          title: 'Add Subscription',
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
                      onPressed: () =>
                          controller.addSubscription(_tagController.text),
                      child: const Text('Subscribe'),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 8),
              ...controller.extraTags.map(
                (tag) => Dismissible(
                  key: ValueKey(tag),
                  direction: DismissDirection.endToStart,
                  background: Container(
                    alignment: Alignment.centerRight,
                    padding: const EdgeInsets.only(right: 16),
                    color: const Color(0xFF7F1D1D),
                    child: const Icon(Icons.delete, color: Colors.white),
                  ),
                  onDismissed: (_) => controller.removeSubscription(tag),
                  child: ListTile(
                    dense: true,
                    leading: const Icon(Icons.tag, size: 18),
                    title: Text(tag),
                    trailing: const Icon(Icons.chevron_right),
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

Future<void> openScanner(
  BuildContext context, {
  required void Function(String value) onResult,
}) async {
  await showModalBottomSheet(
    context: context,
    isScrollControlled: true,
    backgroundColor: const Color(0xFF0B0F17),
  builder: (context) => _QrScannerSheet(onResult: onResult),
  );
}

class _QrScannerSheet extends StatefulWidget {
  final void Function(String value) onResult;

  const _QrScannerSheet({required this.onResult});

  @override
  State<_QrScannerSheet> createState() => _QrScannerSheetState();
}

class _QrScannerSheetState extends State<_QrScannerSheet> {
  bool _handled = false;
  bool _torchOn = false;
  final MobileScannerController _scannerController = MobileScannerController();

  void _handle(String value) {
    if (_handled) return;
    _handled = true;
    widget.onResult(value);
    Navigator.of(context).pop();
  }

  @override
  void dispose() {
    _scannerController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: SizedBox(
        height: MediaQuery.of(context).size.height * 0.6,
        child: Column(
          children: [
            const SizedBox(height: 12),
            Container(
              height: 4,
              width: 40,
              decoration: BoxDecoration(
                color: Colors.white24,
                borderRadius: BorderRadius.circular(999),
              ),
            ),
            const SizedBox(height: 12),
            Text(
              'Scan QR',
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 12),
            Expanded(
              child: ClipRRect(
                borderRadius: BorderRadius.circular(16),
                child: Stack(
                  children: [
                    MobileScanner(
                      controller: _scannerController,
                      onDetect: (capture) {
                        for (final barcode in capture.barcodes) {
                          final raw = barcode.rawValue;
                          if (raw != null && raw.trim().isNotEmpty) {
                            _handle(raw.trim());
                            break;
                          }
                        }
                      },
                    ),
                    Align(
                      alignment: Alignment.center,
                      child: Container(
                        width: 220,
                        height: 220,
                        decoration: BoxDecoration(
                          borderRadius: BorderRadius.circular(16),
                          border: Border.all(
                            color: const Color(0xFF60A5FA),
                            width: 2,
                          ),
                        ),
                      ),
                    ),
                    Positioned(
                      right: 12,
                      top: 12,
                      child: IconButton(
                        icon: Icon(
                          _torchOn ? Icons.flash_on : Icons.flash_off,
                          color: Colors.white,
                        ),
                        onPressed: () async {
                          await _scannerController.toggleTorch();
                          if (mounted) {
                            setState(() => _torchOn = !_torchOn);
                          }
                        },
                      ),
                    ),
                  ],
                ),
              ),
            ),
            const SizedBox(height: 12),
            TextButton(
              onPressed: () => Navigator.of(context).pop(),
              child: const Text('Cancel'),
            ),
            const SizedBox(height: 12),
          ],
        ),
      ),
    );
  }
}
