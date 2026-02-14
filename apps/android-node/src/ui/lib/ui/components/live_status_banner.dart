import 'package:flutter/material.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';

class LiveStatusBanner extends StatelessWidget {
  final SocialController controller;

  const LiveStatusBanner({super.key, required this.controller});

  @override
  Widget build(BuildContext context) {
    final statuses = controller.liveStatuses;
    if (statuses.isEmpty) return const SizedBox.shrink();

    return Container(
      height: 80,
      margin: const EdgeInsets.only(bottom: 16),
      child: ListView.builder(
        scrollDirection: Axis.horizontal,
        itemCount: statuses.length,
        itemBuilder: (context, index) {
          final status = statuses[index];
          final pubkey = status.authorPubkey ?? '';
          final displayName = controller.getDisplayName(pubkey);
          final emoji = status.statusEmoji ?? '💭';
          final profile = controller.nodeService.profiles[pubkey];
          final avatarRoot = profile?.avatarMediaRoot;
          final avatarBytes = avatarRoot != null ? controller.imageCache[avatarRoot] : null;

          return Container(
            width: 70,
            margin: const EdgeInsets.only(right: 12),
            child: Column(
              children: [
                Stack(
                  children: [
                    CircleAvatar(
                      radius: 24,
                      backgroundColor: VeilTheme.accentSubtle,
                      backgroundImage: avatarBytes != null ? MemoryImage(avatarBytes) : null,
                      child: avatarBytes == null 
                        ? Text(displayName.substring(0, 1).toUpperCase())
                        : null,
                    ),
                    Positioned(
                      right: 0,
                      bottom: 0,
                      child: Container(
                        padding: const EdgeInsets.all(2),
                        decoration: const BoxDecoration(
                          color: VeilTheme.surface,
                          shape: BoxShape.circle,
                        ),
                        child: Text(emoji, style: const TextStyle(fontSize: 12)),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 4),
                Text(
                  displayName,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: Theme.of(context).textTheme.labelSmall,
                ),
              ],
            ),
          );
        },
      ),
    );
  }
}
