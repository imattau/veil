import 'package:flutter/material.dart';
import '../theme/veil_theme.dart';

class FeedShimmer extends StatefulWidget {
  const FeedShimmer({super.key});

  @override
  State<FeedShimmer> createState() => _FeedShimmerState();
}

class _FeedShimmerState extends State<FeedShimmer> with SingleTickerProviderStateMixin {
  late AnimationController _controller;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 1500),
    )..repeat();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: _controller,
      builder: (context, child) {
        return Opacity(
          opacity: 0.3 + (0.4 * _controller.value),
          child: child,
        );
      },
      child: ListView.builder(
        itemCount: 5,
        padding: const EdgeInsets.all(16),
        physics: const NeverScrollableScrollPhysics(),
        itemBuilder: (context, index) => const _ShimmerPost(),
      ),
    );
  }
}

class _ShimmerPost extends StatelessWidget {
  const _ShimmerPost();

  @override
  Widget build(BuildContext context) {
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Container(
                  width: 40,
                  height: 40,
                  decoration: const BoxDecoration(
                    color: Colors.white10,
                    shape: BoxShape.circle,
                  ),
                ),
                const SizedBox(width: 12),
                Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Container(width: 100, height: 12, color: Colors.white10),
                    const SizedBox(height: 6),
                    Container(width: 60, height: 10, color: Colors.white10),
                  ],
                ),
              ],
            ),
            const SizedBox(height: 16),
            Container(width: double.infinity, height: 14, color: Colors.white10),
            const SizedBox(height: 8),
            Container(width: double.infinity, height: 14, color: Colors.white10),
            const SizedBox(height: 8),
            Container(width: 200, height: 14, color: Colors.white10),
          ],
        ),
      ),
    );
  }
}
