import 'package:flutter/material.dart';
import '../theme/veil_theme.dart';

class FeedShimmer extends StatefulWidget {
  const FeedShimmer({super.key});

  @override
  State<FeedShimmer> createState() => _FeedShimmerState();
}

class _FeedShimmerState extends State<FeedShimmer>
    with SingleTickerProviderStateMixin {
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
    return ListView.builder(
      itemCount: 5,
      padding: const EdgeInsets.all(16),
      physics: const NeverScrollableScrollPhysics(),
      itemBuilder: (context, index) => _ShimmerPost(controller: _controller),
    );
  }
}

class _ShimmerPost extends StatelessWidget {
  final Animation<double> controller;

  const _ShimmerPost({required this.controller});

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
                _ShimmerBox(
                  width: 40,
                  height: 40,
                  controller: controller,
                  shape: BoxShape.circle,
                ),
                const SizedBox(width: 12),
                Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    _ShimmerBox(
                        width: 100, height: 12, controller: controller),
                    const SizedBox(height: 6),
                    _ShimmerBox(width: 60, height: 10, controller: controller),
                  ],
                ),
              ],
            ),
            const SizedBox(height: 16),
            _ShimmerBox(
                width: double.infinity, height: 14, controller: controller),
            const SizedBox(height: 8),
            _ShimmerBox(
                width: double.infinity, height: 14, controller: controller),
            const SizedBox(height: 8),
            _ShimmerBox(width: 200, height: 14, controller: controller),
          ],
        ),
      ),
    );
  }
}

class _ShimmerBox extends StatelessWidget {
  final double width;
  final double height;
  final Animation<double> controller;
  final BoxShape shape;

  const _ShimmerBox({
    required this.width,
    required this.height,
    required this.controller,
    this.shape = BoxShape.rectangle,
  });

  @override
  Widget build(BuildContext context) {
    return AnimatedBuilder(
      animation: controller,
      builder: (context, child) {
        return Container(
          width: width,
          height: height,
          decoration: BoxDecoration(
            shape: shape,
            borderRadius: shape == BoxShape.rectangle
                ? BorderRadius.circular(4)
                : null,
            gradient: LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: const [
                Colors.white10,
                Colors.white24,
                Colors.white10,
              ],
              stops: [
                (controller.value - 0.3).clamp(0.0, 1.0),
                controller.value.clamp(0.0, 1.0),
                (controller.value + 0.3).clamp(0.0, 1.0),
              ],
            ),
          ),
        );
      },
    );
  }
}
