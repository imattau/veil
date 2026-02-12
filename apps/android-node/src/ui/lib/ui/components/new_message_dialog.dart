import 'package:flutter/material.dart';
import '../../logic/messaging_controller.dart';
import '../../logic/social_controller.dart';
import '../screens/chat_detail_view.dart';
import '../theme/veil_theme.dart';

class NewMessageDialog extends StatefulWidget {
  final MessagingController controller;
  final SocialController socialController;

  const NewMessageDialog({
    super.key,
    required this.controller,
    required this.socialController,
  });

  @override
  State<NewMessageDialog> createState() => _NewMessageDialogState();
}

class _NewMessageDialogState extends State<NewMessageDialog> {
  final TextEditingController _searchController = TextEditingController();
  String _query = '';

  @override
  void dispose() {
    _searchController.dispose();
    super.dispose();
  }

  void _openChat(String pubkey) {
    final title = widget.socialController.getDisplayName(pubkey);
    Navigator.pop(context);
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (context) => ChatDetailView(
          title: title,
          pubkey: pubkey,
          controller: widget.controller,
          socialController: widget.socialController,
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    final recipients = widget.controller.suggestedRecipients(query: _query);
    return AlertDialog(
      title: const Text('New Message'),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          TextField(
            controller: _searchController,
            onChanged: (value) => setState(() => _query = value),
            decoration: const InputDecoration(
              prefixIcon: Icon(Icons.search),
              labelText: 'Find contact',
              hintText: 'Search people you follow / messaged',
              labelStyle: TextStyle(color: VeilTheme.textSecondary),
            ),
          ),
          const SizedBox(height: 16),
          const Text(
            'Suggested',
            style: TextStyle(
              color: VeilTheme.textSecondary,
              fontSize: 12,
              fontWeight: FontWeight.w600,
            ),
          ),
          const SizedBox(height: 8),
          SizedBox(
            height: 260,
            width: double.maxFinite,
            child: recipients.isEmpty
                ? const Center(
                    child: Text(
                      'No known recipients yet.\nFollow people or receive DMs first.',
                      textAlign: TextAlign.center,
                      style: TextStyle(color: VeilTheme.textSecondary),
                    ),
                  )
                : ListView.builder(
                    itemCount: recipients.length,
                    itemBuilder: (context, index) {
                      final pubkey = recipients[index];
                      final name = widget.socialController.getDisplayName(
                        pubkey,
                      );
                      final messages = widget.controller.getMessagesForContact(
                        pubkey,
                      );
                      final last = messages.isNotEmpty
                          ? widget.controller.getMessageContent(messages.first)
                          : null;
                      final unread = widget.controller.getUnreadCountForThread(
                        isGroup: false,
                        id: pubkey,
                      );
                      return ListTile(
                        leading: CircleAvatar(
                          backgroundColor: VeilTheme.accent.withOpacity(0.1),
                          child: Text(name.substring(0, 1).toUpperCase()),
                        ),
                        title: Text(name),
                        subtitle: Text(
                          last ??
                              (pubkey.length >= 16
                                  ? '${pubkey.substring(0, 8)}...${pubkey.substring(pubkey.length - 8)}'
                                  : pubkey),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontFamily: last == null ? 'monospace' : null,
                            fontSize: 12,
                            color: last == null
                                ? VeilTheme.textSecondary
                                : VeilTheme.textPrimary,
                          ),
                        ),
                        trailing: unread > 0
                            ? CircleAvatar(
                                radius: 11,
                                backgroundColor: VeilTheme.accent,
                                child: Text(
                                  unread > 99 ? '99+' : '$unread',
                                  style: const TextStyle(
                                    color: Colors.black,
                                    fontSize: 10,
                                    fontWeight: FontWeight.w700,
                                  ),
                                ),
                              )
                            : null,
                        onTap: () => _openChat(pubkey),
                      );
                    },
                  ),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
      ],
    );
  }
}
