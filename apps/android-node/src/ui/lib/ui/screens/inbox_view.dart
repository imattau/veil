import 'package:flutter/material.dart';
import '../../logic/messaging_controller.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';
import '../components/empty_state.dart';
import './chat_detail_view.dart';

class InboxView extends StatelessWidget {
  final MessagingController controller;
  final SocialController socialController;

  const InboxView({
    super.key,
    required this.controller,
    required this.socialController,
  });

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: controller,
      builder: (context, _) {
        final threads = controller.conversations;

        if (threads.isEmpty) {
          return const EmptyState(
            icon: Icons.mail_outline,
            title: 'No messages yet',
            message:
                'Your private conversations and group chats will appear here.',
          );
        }

        return ListView.builder(
          padding: const EdgeInsets.symmetric(vertical: 8),
          itemCount: threads.length,
          itemBuilder: (context, index) {
            final thread = threads[index];
            return _ConversationTile(
              thread: thread,
              controller: controller,
              socialController: socialController,
            );
          },
        );
      },
    );
  }
}

class _ConversationTile extends StatelessWidget {
  final ConversationThread thread;
  final MessagingController controller;
  final SocialController socialController;

  const _ConversationTile({
    required this.thread,
    required this.controller,
    required this.socialController,
  });

  @override
  Widget build(BuildContext context) {
    final displayName = thread.isGroup
        ? _groupLabel(thread.id)
        : socialController.getDisplayName(thread.id);
    final content = thread.lastMessage != null
        ? controller.getMessageContent(thread.lastMessage!)
        : 'Encrypted message';

    final isUnread = thread.unreadCount > 0;
    final time = thread.lastMessage?.createdAt != null
        ? _formatTime(
            DateTime.fromMillisecondsSinceEpoch(
              thread.lastMessage!.createdAt! * 1000,
            ),
          )
        : '';
    final isOutgoing =
        thread.lastMessage?.authorPubkey == controller.nodeService.state.identityHex;

    return ListTile(
      contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 4),
      leading: Stack(
        children: [
          CircleAvatar(
            radius: 24,
            backgroundColor: thread.isGroup
                ? Colors.white.withOpacity(0.08)
                : VeilTheme.accent.withOpacity(0.18),
            child: thread.isGroup
                ? const Icon(Icons.group, color: VeilTheme.textSecondary)
                : Text(displayName.substring(0, 1).toUpperCase()),
          ),
          if (isUnread)
            const Positioned(
              right: 0,
              top: 0,
              child: CircleAvatar(radius: 5, backgroundColor: VeilTheme.accent),
            ),
        ],
      ),
      title: Text(
        displayName,
        style: TextStyle(
          fontWeight: isUnread ? FontWeight.w700 : FontWeight.w600,
        ),
      ),
      subtitle: Text(
        '${isOutgoing ? 'You: ' : ''}${content ?? 'Decrypting...'}',
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(
          color: content == null ? VeilTheme.accent : VeilTheme.textSecondary,
          fontWeight: isUnread ? FontWeight.w600 : FontWeight.w400,
        ),
      ),
      trailing: Column(
        mainAxisAlignment: MainAxisAlignment.center,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          if (time.isNotEmpty)
            Text(
              time,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                color: isUnread ? VeilTheme.textPrimary : VeilTheme.textSecondary,
              ),
            ),
          if (thread.unreadCount > 0) ...[
            const SizedBox(height: 6),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
              decoration: BoxDecoration(
                color: VeilTheme.accent,
                borderRadius: BorderRadius.circular(12),
              ),
              child: Text(
                thread.unreadCount > 99 ? '99+' : '${thread.unreadCount}',
                style: const TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w700,
                  color: Colors.black,
                ),
              ),
            ),
          ],
        ],
      ),
      onTap: () {
        controller.markThreadRead(isGroup: thread.isGroup, id: thread.id);
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (context) => ChatDetailView(
              title: displayName,
              pubkey: thread.isGroup ? null : thread.id,
              groupId: thread.isGroup ? thread.id : null,
              controller: controller,
              socialController: socialController,
            ),
          ),
        );
      },
    );
  }

  String _formatTime(DateTime time) {
    final diff = DateTime.now().difference(time);
    if (diff.inMinutes < 1) return 'now';
    if (diff.inMinutes < 60) return '${diff.inMinutes}m';
    if (diff.inHours < 24) return '${diff.inHours}h';
    if (diff.inDays < 7) return '${diff.inDays}d';
    return '${time.month}/${time.day}';
  }

  String _groupLabel(String id) {
    if (id.length <= 12) {
      return id;
    }
    return 'Group ${id.substring(0, 8)}';
  }
}
