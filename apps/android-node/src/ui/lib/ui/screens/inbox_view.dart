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
        final contacts = controller.directMessageContacts;
        final groups = controller.groupIds;

        if (contacts.isEmpty && groups.isEmpty) {
          return const EmptyState(
            icon: Icons.mail_outline,
            title: 'No messages yet',
            message: 'Your private conversations and group chats will appear here.',
          );
        }

        return ListView(
          children: [
            if (groups.isNotEmpty) ...[
              const _SectionHeader(title: 'Groups'),
              ...groups.map((id) => _GroupTile(
                    groupId: id,
                    controller: controller,
                  )),
            ],
            if (contacts.isNotEmpty) ...[
              const _SectionHeader(title: 'Direct Messages'),
              ...contacts.map((pubkey) => _ContactTile(
                    pubkey: pubkey,
                    controller: controller,
                    socialController: socialController,
                  )),
            ],
          ],
        );
      },
    );
  }
}

class _SectionHeader extends StatelessWidget {
  final String title;
  const _SectionHeader({required this.title});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 24, 16, 8),
      child: Text(
        title.toUpperCase(),
        style: Theme.of(context).textTheme.labelSmall?.copyWith(
              fontWeight: FontWeight.bold,
              letterSpacing: 1.2,
            ),
      ),
    );
  }
}

class _ContactTile extends StatelessWidget {
  final String pubkey;
  final MessagingController controller;
  final SocialController socialController;

  const _ContactTile({
    required this.pubkey,
    required this.controller,
    required this.socialController,
  });

  @override
  Widget build(BuildContext context) {
    final displayName = socialController.getDisplayName(pubkey);
    final messages = controller.getMessagesForContact(pubkey);
    final lastMessage = messages.isNotEmpty ? messages.first : null;
    final content = lastMessage != null ? controller.getMessageContent(lastMessage) : 'Encrypted message';

    return ListTile(
      leading: CircleAvatar(
        backgroundColor: VeilTheme.accent.withOpacity(0.1),
        child: Text(displayName.substring(0, 1).toUpperCase()),
      ),
      title: Text(displayName, style: const TextStyle(fontWeight: FontWeight.bold)),
      subtitle: Text(
        content ?? 'Decrypting...',
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
        style: TextStyle(color: content == null ? VeilTheme.accent : VeilTheme.textSecondary),
      ),
      onTap: () {
        Navigator.push(
          context,
          MaterialPageRoute(
            builder: (context) => ChatDetailView(
              title: displayName,
              pubkey: pubkey,
              controller: controller,
              socialController: socialController,
            ),
          ),
        );
      },
    );
  }
}

class _GroupTile extends StatelessWidget {
  final String groupId;
  final MessagingController controller;

  const _GroupTile({required this.groupId, required this.controller});

  @override
  Widget build(BuildContext context) {
    final messages = controller.getMessagesForGroup(groupId);
    final lastMessage = messages.isNotEmpty ? messages.first : null;
    final content = lastMessage != null ? controller.getMessageContent(lastMessage) : null;

    return ListTile(
      leading: Container(
        width: 40,
        height: 40,
        decoration: BoxDecoration(
          color: Colors.white.withOpacity(0.05),
          borderRadius: BorderRadius.circular(8),
        ),
        child: const Icon(Icons.group, color: VeilTheme.textSecondary),
      ),
      title: Text(groupId, style: const TextStyle(fontWeight: FontWeight.bold)),
      subtitle: Text(
        content ?? 'Encrypted group message',
        maxLines: 1,
        overflow: TextOverflow.ellipsis,
      ),
      onTap: () {
        // TODO: Navigate to group chat
      },
    );
  }
}
