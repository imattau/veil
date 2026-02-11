import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../logic/messaging_controller.dart';
import '../../logic/social_controller.dart';
import '../../logic/models/node_event.dart';
import '../theme/veil_theme.dart';

class ChatDetailView extends StatelessWidget {
  final String title;
  final String? pubkey;
  final String? groupId;
  final MessagingController controller;
  final SocialController socialController;

  const ChatDetailView({
    super.key,
    required this.title,
    this.pubkey,
    this.groupId,
    required this.controller,
    required this.socialController,
  });

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: Text(title),
      ),
      body: Column(
        children: [
          Expanded(
            child: ListenableBuilder(
              listenable: controller,
              builder: (context, _) {
                final messages = pubkey != null
                    ? controller.getMessagesForContact(pubkey!)
                    : controller.getMessagesForGroup(groupId!);

                // Sort by seq ascending for chat flow
                final sorted = messages.toList()..sort((a, b) => a.seq.compareTo(b.seq));

                return ListView.builder(
                  padding: const EdgeInsets.all(16),
                  itemCount: sorted.length,
                  itemBuilder: (context, index) {
                    final msg = sorted[index];
                    final isMe = msg.authorPubkey == controller.nodeService.state.identityHex;
                    final content = controller.getMessageContent(msg);

                    return _MessageBubble(
                      content: content ?? 'Decrypting...',
                      isMe: isMe,
                      time: msg.createdAt != null
                          ? DateTime.fromMillisecondsSinceEpoch(msg.createdAt! * 1000)
                          : null,
                    );
                  },
                );
              },
            ),
          ),
          _ChatInputBar(
            onSend: (text) {
              // TODO: Implement encrypted send in node_service
            },
          ),
        ],
      ),
    );
  }
}

class _MessageBubble extends StatelessWidget {
  final String content;
  final bool isMe;
  final DateTime? time;

  const _MessageBubble({
    required this.content,
    required this.isMe,
    this.time,
  });

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: isMe ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.only(bottom: 12),
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 10),
        constraints: BoxConstraints(maxWidth: MediaQuery.of(context).size.width * 0.75),
        decoration: BoxDecoration(
          color: isMe ? VeilTheme.accent : VeilTheme.surface,
          borderRadius: BorderRadius.circular(18).copyWith(
            bottomRight: isMe ? const Radius.circular(0) : null,
            bottomLeft: !isMe ? const Radius.circular(0) : null,
          ),
        ),
        child: Column(
          crossAxisAlignment: isMe ? CrossAxisAlignment.end : CrossAxisAlignment.start,
          children: [
            Text(
              content,
              style: TextStyle(
                color: isMe ? Colors.black : VeilTheme.textPrimary,
                fontSize: 15,
              ),
            ),
            if (time != null) ...[
              const SizedBox(height: 4),
              Text(
                '${time!.hour}:${time!.minute.toString().padLeft(2, '0')}',
                style: TextStyle(
                  fontSize: 10,
                  color: isMe ? Colors.black.withOpacity(0.5) : VeilTheme.textSecondary,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }
}

class _ChatInputBar extends StatefulWidget {
  final Function(String) onSend;

  const _ChatInputBar({required this.onSend});

  @override
  State<_ChatInputBar> createState() => _ChatInputBarState();
}

class _ChatInputBarState extends State<_ChatInputBar> {
  final TextEditingController _controller = TextEditingController();

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: EdgeInsets.only(
        bottom: MediaQuery.of(context).viewInsets.bottom + 12,
        left: 16,
        right: 16,
        top: 12,
      ),
      decoration: const BoxDecoration(
        color: VeilTheme.background,
        border: Border(top: BorderSide(color: Colors.white10)),
      ),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              controller: _controller,
              decoration: InputDecoration(
                hintText: 'Message',
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(24),
                  borderSide: BorderSide.none,
                ),
                fillColor: VeilTheme.surface,
                filled: true,
                contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
              ),
            ),
          ),
          const SizedBox(width: 8),
          IconButton(
            onPressed: () {
              final text = _controller.text.trim();
              if (text.isNotEmpty) {
                HapticFeedback.lightImpact();
                widget.onSend(text);
                _controller.clear();
              }
            },
            icon: const Icon(Icons.send, color: VeilTheme.accent),
          ),
        ],
      ),
    );
  }
}
