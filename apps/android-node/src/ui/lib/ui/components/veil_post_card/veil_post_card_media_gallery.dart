part of '../veil_post_card.dart';

class _PostMediaGallery extends StatelessWidget {
  final List<String> mediaRoots;
  final SocialController controller;

  const _PostMediaGallery({required this.mediaRoots, required this.controller});

  @override
  Widget build(BuildContext context) {
    if (mediaRoots.isEmpty) return const SizedBox.shrink();

    if (mediaRoots.length == 1) {
      return _buildImage(context, mediaRoots[0], height: 220);
    }

    if (mediaRoots.length == 2) {
      return Row(
        children: [
          Expanded(child: _buildImage(context, mediaRoots[0], height: 160)),
          const SizedBox(width: 4),
          Expanded(child: _buildImage(context, mediaRoots[1], height: 160)),
        ],
      );
    }

    if (mediaRoots.length == 3) {
      return Column(
        children: [
          _buildImage(context, mediaRoots[0], height: 160),
          const SizedBox(height: 4),
          Row(
            children: [
              Expanded(child: _buildImage(context, mediaRoots[1], height: 120)),
              const SizedBox(width: 4),
              Expanded(child: _buildImage(context, mediaRoots[2], height: 120)),
            ],
          ),
        ],
      );
    }

    final display = mediaRoots.take(4).toList();
    return Column(
      children: [
        Row(
          children: [
            Expanded(child: _buildImage(context, display[0], height: 120)),
            const SizedBox(width: 4),
            Expanded(child: _buildImage(context, display[1], height: 120)),
          ],
        ),
        const SizedBox(height: 4),
        Row(
          children: [
            Expanded(child: _buildImage(context, display[2], height: 120)),
            const SizedBox(width: 4),
            Expanded(child: _buildImage(context, display[3], height: 120)),
          ],
        ),
      ],
    );
  }

  Widget _buildImage(BuildContext context, String root, {double? height}) {
    final bytes = controller.imageCache[root];
    return Container(
      clipBehavior: Clip.antiAlias,
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(12),
        color: Colors.white.withValues(alpha: 0.03),
      ),
      child: bytes != null
          ? GestureDetector(
              onTap: () {
                showDialog(
                  context: context,
                  builder: (context) => Dialog.fullscreen(
                    backgroundColor: Colors.black,
                    child: Stack(
                      children: [
                        InteractiveViewer(
                          child: Center(child: Image.memory(bytes)),
                        ),
                        Positioned(
                          top: 40,
                          right: 20,
                          child: IconButton(
                            icon: const Icon(
                              Icons.close,
                              color: Colors.white,
                              size: 30,
                            ),
                            onPressed: () => Navigator.pop(context),
                          ),
                        ),
                      ],
                    ),
                  ),
                );
              },
              child: Image.memory(
                bytes,
                fit: BoxFit.cover,
                width: double.infinity,
                height: height,
                errorBuilder: (context, error, stackTrace) =>
                    _mediaUnavailable(height),
              ),
            )
          : SizedBox(
              height: height,
              child: Center(
                child: CircularProgressIndicator(
                  strokeWidth: 2,
                  color: VeilTheme.accent.withValues(alpha: 0.8),
                ),
              ),
            ),
    );
  }

  Widget _mediaUnavailable(double? height) {
    return SizedBox(
      height: height,
      child: const Center(
        child: Text(
          'Media unavailable',
          style: TextStyle(color: VeilTheme.textSecondary, fontSize: 10),
        ),
      ),
    );
  }
}
