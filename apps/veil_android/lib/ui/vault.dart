part of 'package:veil_android/main.dart';
class VaultView extends StatelessWidget {
  final VeilAppController controller;

  const VaultView({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    final remaining = controller.epochRemainingSeconds;
    final hours = (remaining ~/ 3600).toString().padLeft(2, '0');
    final minutes = ((remaining % 3600) ~/ 60).toString().padLeft(2, '0');
    final seconds = (remaining % 60).toString().padLeft(2, '0');
    return ListView(
      padding: const EdgeInsets.all(16),
      children: [
        _Panel(
          title: 'Private Vault',
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Encrypted conversations will appear here. Rotating rendezvous tags keep private circles private.',
              ),
              const SizedBox(height: 12),
              Text(
                'Next rotation in $hours:$minutes:$seconds',
                style: Theme.of(
                  context,
                ).textTheme.bodyMedium?.copyWith(color: Colors.white70),
              ),
              if (controller.epochOverlapActive) ...[
                const SizedBox(height: 8),
                Row(
                  children: [
                    const Icon(
                      Icons.swap_horiz,
                      size: 16,
                      color: Color(0xFF38BDF8),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      'Overlap window active',
                      style: Theme.of(
                        context,
                      ).textTheme.bodySmall?.copyWith(color: Colors.white60),
                    ),
                  ],
                ),
              ],
            ],
          ),
        ),
      ],
    );
  }
}

