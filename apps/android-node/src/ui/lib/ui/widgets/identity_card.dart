import 'package:flutter/material.dart';

class IdentityCard extends StatelessWidget {
  final String? identityHex;
  final VoidCallback? onRotate;
  final VoidCallback? onExport;
  final VoidCallback? onImport;
  final bool busy;

  const IdentityCard({
    super.key,
    required this.identityHex,
    required this.onRotate,
    this.onExport,
    this.onImport,
    required this.busy,
  });

  @override
  Widget build(BuildContext context) {
    final value = identityHex ?? 'Not available';
    return Card(
      elevation: 0,
      color: Colors.white,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              'Node Identity',
              style: Theme.of(context).textTheme.titleMedium,
            ),
            const SizedBox(height: 8),
            SelectableText(
              value,
              style: const TextStyle(fontFamily: 'monospace'),
            ),
            const SizedBox(height: 12),
            Row(
              children: [
                TextButton.icon(
                  onPressed: busy ? null : onRotate,
                  icon: const Icon(Icons.refresh),
                  label: const Text('Rotate'),
                ),
                const SizedBox(width: 8),
                TextButton.icon(
                  onPressed: busy ? null : onExport,
                  icon: const Icon(Icons.download),
                  label: const Text('Export'),
                ),
                const SizedBox(width: 8),
                TextButton.icon(
                  onPressed: busy ? null : onImport,
                  icon: const Icon(Icons.upload),
                  label: const Text('Import'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }
}
