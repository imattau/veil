import 'package:flutter/material.dart';

class IdentityCard extends StatelessWidget {
  final String? identityHex;
  final VoidCallback? onRotate;
  final bool busy;

  const IdentityCard({
    super.key,
    required this.identityHex,
    required this.onRotate,
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
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton.icon(
                onPressed: busy ? null : onRotate,
                icon: const Icon(Icons.refresh),
                label: const Text('Rotate identity'),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
