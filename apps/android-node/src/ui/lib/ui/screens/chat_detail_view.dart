import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import '../../logic/messaging_controller.dart';
import '../../logic/social_controller.dart';
import '../../logic/models/node_event.dart';
import '../theme/veil_theme.dart';

class ChatDetailView extends StatefulWidget {
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
  State<ChatDetailView> createState() => _ChatDetailViewState();
}

class _ChatDetailViewState extends State<ChatDetailView> {
  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_markThreadRead);
    _markThreadRead();
  }

  @override
  void didUpdateWidget(covariant ChatDetailView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.pubkey != widget.pubkey || oldWidget.groupId != widget.groupId) {
      _markThreadRead();
    }
  }

  @override
  void dispose() {
    widget.controller.removeListener(_markThreadRead);
    super.dispose();
  }

  void _markThreadRead() {
    final id = widget.groupId ?? widget.pubkey;
    if (id == null) return;
    widget.controller.markThreadRead(
      isGroup: widget.groupId != null,
      id: id,
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(widget.title)),
      body: Column(
        children: [
          Expanded(
            child: ListenableBuilder(
              listenable: widget.controller,
              builder: (context, _) {
                final messages = widget.pubkey != null
                    ? widget.controller.getMessagesForContact(widget.pubkey!)
                    : widget.controller.getMessagesForGroup(widget.groupId!);

                // Sort by seq ascending for chat flow
                final sorted = messages.toList()
                  ..sort((a, b) => a.seq.compareTo(b.seq));

                return ListView.builder(
                  padding: const EdgeInsets.fromLTRB(12, 16, 12, 8),
                  itemCount: sorted.length,
                  itemBuilder: (context, index) {
                    final msg = sorted[index];
                    final isMe =
                        msg.authorPubkey ==
                        widget.controller.nodeService.state.identityHex;
                    final content = widget.controller.getMessageContent(msg);

                    return _MessageBubble(
                      content: content ?? 'Decrypting...',
                      isMe: isMe,
                      senderLabel: isMe
                          ? 'You'
                          : widget.socialController.getDisplayName(
                              msg.authorPubkey ?? '',
                            ),
                      time: msg.createdAt != null
                          ? DateTime.fromMillisecondsSinceEpoch(
                              msg.createdAt! * 1000,
                            )
                          : null,
                    );
                  },
                );
              },
            ),
          ),
          _ChatInputBar(
            onSend: (text) async {
              if (widget.pubkey != null) {
                await widget.controller.publishDirectMessage(
                  recipientPubkey: widget.pubkey!,
                  text: text,
                );
              } else if (widget.groupId != null) {
                await widget.controller.publishGroupMessage(
                  groupId: widget.groupId!,
                  text: text,
                );
              }
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
  final String senderLabel;
  final DateTime? time;

  const _MessageBubble({
    required this.content,
    required this.isMe,
    required this.senderLabel,
    this.time,
  });

  @override
  Widget build(BuildContext context) {
    return Align(
      alignment: isMe ? Alignment.centerRight : Alignment.centerLeft,
      child: Container(
        margin: const EdgeInsets.only(bottom: 8),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 10),
        constraints: BoxConstraints(
          maxWidth: MediaQuery.of(context).size.width * 0.78,
        ),
        decoration: BoxDecoration(
          color: isMe ? VeilTheme.accent : VeilTheme.surface,
          borderRadius: BorderRadius.circular(18).copyWith(
            bottomRight: isMe ? const Radius.circular(0) : null,
            bottomLeft: !isMe ? const Radius.circular(0) : null,
          ),
        ),
        child: Column(
          crossAxisAlignment: isMe
              ? CrossAxisAlignment.end
              : CrossAxisAlignment.start,
          children: [
            if (!isMe) ...[
              Text(
                senderLabel,
                style: const TextStyle(
                  fontSize: 11,
                  fontWeight: FontWeight.w600,
                  color: VeilTheme.textSecondary,
                ),
              ),
              const SizedBox(height: 3),
            ],
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
                _formatTime(time!),
                style: TextStyle(
                  fontSize: 10,
                  color: isMe
                      ? Colors.black.withOpacity(0.5)
                      : VeilTheme.textSecondary,
                ),
              ),
            ],
          ],
        ),
      ),
    );
  }

  String _formatTime(DateTime dateTime) {
    final h = dateTime.hour % 12 == 0 ? 12 : dateTime.hour % 12;
    final m = dateTime.minute.toString().padLeft(2, '0');
    final suffix = dateTime.hour >= 12 ? 'PM' : 'AM';
    return '$h:$m $suffix';
  }
}

class _ChatInputBar extends StatefulWidget {
  final Future<void> Function(String) onSend;

  const _ChatInputBar({required this.onSend});

  @override
  State<_ChatInputBar> createState() => _ChatInputBarState();
}

class _ChatInputBarState extends State<_ChatInputBar> {
  final TextEditingController _controller = TextEditingController();
  bool _sending = false;

  @override
  void initState() {
    super.initState();
    _controller.addListener(_onTextChanged);
  }

  @override
  void dispose() {
    _controller.removeListener(_onTextChanged);
    _controller.dispose();
    super.dispose();
  }

  void _onTextChanged() {
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final canSend = !_sending && _controller.text.trim().isNotEmpty;
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
                contentPadding: const EdgeInsets.symmetric(
                  horizontal: 16,
                  vertical: 10,
                ),
              ),
              textCapitalization: TextCapitalization.sentences,
              minLines: 1,
              maxLines: 4,
            ),
          ),
          const SizedBox(width: 8),
          Container(
            decoration: BoxDecoration(
              color: canSend ? VeilTheme.accent : Colors.white10,
              shape: BoxShape.circle,
            ),
            child: IconButton(
              onPressed: canSend
                  ? () async {
                      final text = _controller.text.trim();
                      if (text.isEmpty) return;
                      HapticFeedback.lightImpact();
                      setState(() => _sending = true);
                      try {
                        await widget.onSend(text);
                        _controller.clear();
                      } finally {
                        if (mounted) {
                          setState(() => _sending = false);
                        }
                      }
                    }
                  : null,
              icon: Icon(
                Icons.send_rounded,
                color: canSend ? Colors.black : VeilTheme.textSecondary,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
