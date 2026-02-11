import 'package:flutter/material.dart';

class ServiceControls extends StatelessWidget {
  final bool busy;
  final VoidCallback onStart;
  final VoidCallback onStop;
  final VoidCallback onRefresh;

  const ServiceControls({
    super.key,
    required this.busy,
    required this.onStart,
    required this.onStop,
    required this.onRefresh,
  });

  @override
  Widget build(BuildContext context) {
    return Card(
      elevation: 0,
      color: Colors.white,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Expanded(
              child: ElevatedButton(
                onPressed: busy ? null : onStart,
                style: ElevatedButton.styleFrom(
                  backgroundColor: const Color(0xFF0B1D26),
                  foregroundColor: Colors.white,
                ),
                child: const Text('Start'),
              ),
            ),
            const SizedBox(width: 12),
            Expanded(
              child: OutlinedButton(
                onPressed: busy ? null : onStop,
                child: const Text('Stop'),
              ),
            ),
            const SizedBox(width: 12),
            IconButton(
              onPressed: busy ? null : onRefresh,
              icon: const Icon(Icons.refresh),
              tooltip: 'Refresh',
            ),
          ],
        ),
      ),
    );
  }
}
