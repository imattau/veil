part of 'package:veil_android/main.dart';
class _Panel extends StatelessWidget {
  final String title;
  final Widget child;

  const _Panel({required this.title, required this.child});

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        gradient: LinearGradient(
          colors: [
            scheme.surfaceContainerHighest.withOpacity(0.9),
            const Color(0xFF0B1220),
          ],
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
        ),
        borderRadius: BorderRadius.circular(16),
        border: Border.all(color: const Color(0xFF1F2937)),
        boxShadow: [
          BoxShadow(
            color: Colors.black.withOpacity(0.25),
            blurRadius: 16,
            offset: const Offset(0, 8),
          ),
        ],
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(
            title.toUpperCase(),
            style: Theme.of(context).textTheme.labelLarge?.copyWith(
              letterSpacing: 2,
              color: Colors.white70,
            ),
          ),
          const SizedBox(height: 12),
          child,
        ],
      ),
    );
  }
}

class _InputField extends StatelessWidget {
  final String label;
  final TextEditingController controller;
  final ValueChanged<String>? onChanged;
  final String? errorText;
  final VoidCallback? onScan;

  const _InputField({
    required this.label,
    required this.controller,
    this.onChanged,
    this.errorText,
    this.onScan,
  });

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: TextField(
        controller: controller,
        onChanged: onChanged,
        decoration: InputDecoration(
          labelText: label,
          errorText: errorText,
          suffixIcon: onScan == null
              ? null
              : IconButton(
                  icon: const Icon(Icons.qr_code_scanner),
                  onPressed: onScan,
                ),
          border: OutlineInputBorder(borderRadius: BorderRadius.circular(12)),
        ),
      ),
    );
  }
}


class _BlurPlaceholder extends StatefulWidget {
  final String? blurHash;

  const _BlurPlaceholder({this.blurHash});

  @override
  State<_BlurPlaceholder> createState() => _BlurPlaceholderState();
}

class _BlurPlaceholderState extends State<_BlurPlaceholder>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 2),
  )..repeat(reverse: true);

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.blurHash != null) {
      return ClipRRect(
        borderRadius: BorderRadius.circular(16),
        child: BlurHash(
          hash: widget.blurHash!,
          duration: const Duration(milliseconds: 300),
        ),
      );
    }
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, _) {
        final t = _controller.value;
        return Container(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(16),
            gradient: LinearGradient(
              colors: [
                Color.lerp(
                  const Color(0xFF111827),
                  const Color(0xFF1E293B),
                  t,
                )!,
                Color.lerp(
                  const Color(0xFF0B1220),
                  const Color(0xFF1F2937),
                  t,
                )!,
              ],
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
            ),
          ),
        );
      },
    );
  }
}


class _ShardProgressRing extends StatelessWidget {
  final int have;
  final int total;

  const _ShardProgressRing({required this.have, required this.total});

  @override
  Widget build(BuildContext context) {
    final progress = total == 0 ? 0.0 : have / total;
    return Container(
      margin: const EdgeInsets.all(12),
      padding: const EdgeInsets.all(6),
      decoration: BoxDecoration(
        color: const Color(0xFF0B1220).withOpacity(0.85),
        borderRadius: BorderRadius.circular(999),
        border: Border.all(color: const Color(0xFF1F2937)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: 28,
            height: 28,
            child: CircularProgressIndicator(
              value: progress,
              strokeWidth: 3,
              color: const Color(0xFF34D399),
              backgroundColor: const Color(0xFF1F2937),
            ),
          ),
          const SizedBox(width: 8),
          Text(
            '$have/$total',
            style: Theme.of(
              context,
            ).textTheme.labelMedium?.copyWith(color: Colors.white70),
          ),
        ],
      ),
    );
  }
}
