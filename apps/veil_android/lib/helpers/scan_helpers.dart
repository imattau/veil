import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';

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
                  alignment: Alignment.center,
                  children: [
                    MobileScanner(
                      controller: _scannerController,
                      onDetect: (capture) {
                        for (final barcode in capture.barcodes) {
                          final value = barcode.rawValue;
                          if (value != null) {
                            _handle(value);
                            break;
                          }
                        }
                      },
                    ),
                    Container(
                      margin: const EdgeInsets.all(24),
                      decoration: BoxDecoration(
                        border: Border.all(color: Colors.white54, width: 2),
                        borderRadius: BorderRadius.circular(18),
                      ),
                    ),
                    Positioned(
                      right: 12,
                      top: 12,
                      child: IconButton(
                        onPressed: () {
                          setState(() => _torchOn = !_torchOn);
                          _scannerController.toggleTorch();
                        },
                        icon: Icon(
                          _torchOn ? Icons.flash_on : Icons.flash_off,
                          color: Colors.white70,
                        ),
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
