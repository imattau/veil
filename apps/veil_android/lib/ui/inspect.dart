part of 'package:veil_android/main.dart';
class _InspectSheet extends StatelessWidget {
  final FeedEntry entry;

  const _InspectSheet({required this.entry});

  @override
  Widget build(BuildContext context) {
    return SafeArea(
      child: ClipRRect(
        borderRadius: const BorderRadius.vertical(top: Radius.circular(24)),
        child: BackdropFilter(
          filter: ImageFilter.blur(sigmaX: 14, sigmaY: 14),
          child: Container(
            decoration: BoxDecoration(
              color: Theme.of(context).colorScheme.surface.withOpacity(0.85),
            ),
            child: ListView(
              padding: const EdgeInsets.all(16),
              shrinkWrap: true,
              children: [
                Text('Inspect Post', style: Theme.of(context).textTheme.titleLarge),
                const SizedBox(height: 12),
                _InspectRow(label: 'object_root', value: entry.id),
                _InspectRow(
                  label: 'status',
                  value: entry.reconstructed ? 'reconstructed' : 'pending',
                ),
                _InspectRow(
                  label: 'shards',
                  value: '${entry.shardsHave}/${entry.shardsTotal}',
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _InspectRow extends StatelessWidget {
  final String label;
  final String value;

  const _InspectRow({required this.label, required this.value});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 10),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 110,
            child: Text(
              label,
              style: Theme.of(context).textTheme.bodySmall?.copyWith(
                color: Colors.white70,
              ),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: Theme.of(context).textTheme.bodyMedium,
            ),
          ),
        ],
      ),
    );
  }
}

