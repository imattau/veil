import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../logic/messaging_controller.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import 'chat_detail_view/chat_input_bar.dart';
import 'chat_detail_view/chat_message_bubble.dart';

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
  NodeEvent? _replyTarget;

  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_markThreadRead);
    _markThreadRead();
  }

  @override
  void didUpdateWidget(covariant ChatDetailView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.pubkey != widget.pubkey ||
        oldWidget.groupId != widget.groupId) {
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
    widget.controller.markThreadRead(isGroup: widget.groupId != null, id: id);
  }

  String _senderLabel(NodeEvent message) {
    final self = widget.controller.nodeService.state.identityHex;
    return message.authorPubkey == self
        ? 'You'
        : widget.socialController.getDisplayName(message.authorPubkey ?? '');
  }

  String _previewContent(NodeEvent message) {
    final content = widget.controller.getMessageContent(message)?.trim();
    if (content != null && content.isNotEmpty) {
      return content;
    }
    return 'Encrypted message';
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

                final sorted = messages.toList()
                  ..sort((a, b) => b.seq.compareTo(a.seq));

                final messagesByRoot = <String, NodeEvent>{};
                for (final message in sorted) {
                  final root = message.objectRoot;
                  if (root != null && root.isNotEmpty) {
                    messagesByRoot[root] = message;
                  }
                }

                return ListView.builder(
                  reverse: true,
                  padding: const EdgeInsets.fromLTRB(12, 16, 12, 8),
                  itemCount: sorted.length,
                  itemBuilder: (context, index) {
                    final msg = sorted[index];
                    final isMe =
                        msg.authorPubkey ==
                        widget.controller.nodeService.state.identityHex;
                    final content = widget.controller.getMessageContent(msg);
                    final replied = msg.replyToRoot == null
                        ? null
                        : messagesByRoot[msg.replyToRoot!];

                    return ChatMessageBubble(
                      key: Key('chat-message-${msg.objectRoot ?? msg.seq}'),
                      content: content ?? 'Decrypting...',
                      isMe: isMe,
                      senderLabel: isMe
                          ? 'You'
                          : widget.socialController.getDisplayName(
                              msg.authorPubkey ?? '',
                            ),
                      replySenderLabel: replied == null
                          ? null
                          : _senderLabel(replied),
                      replyPreview: replied == null
                          ? null
                          : _previewContent(replied),
                      onLongPress: () {
                        HapticFeedback.selectionClick();
                        setState(() => _replyTarget = msg);
                      },
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
          ChatInputBar(
            replyTargetLabel: _replyTarget == null
                ? null
                : _senderLabel(_replyTarget!),
            replyTargetPreview: _replyTarget == null
                ? null
                : _previewContent(_replyTarget!),
            onClearReplyTarget: _replyTarget == null
                ? null
                : () => setState(() => _replyTarget = null),
            onSend: (text) async {
              final replyToRoot = _replyTarget?.objectRoot;
              if (widget.pubkey != null) {
                await widget.controller.publishDirectMessage(
                  recipientPubkey: widget.pubkey!,
                  text: text,
                  replyToRoot: replyToRoot,
                );
              } else if (widget.groupId != null) {
                await widget.controller.publishGroupMessage(
                  groupId: widget.groupId!,
                  text: text,
                  replyToRoot: replyToRoot,
                );
              }
              if (mounted) {
                setState(() => _replyTarget = null);
              }
            },
          ),
        ],
      ),
    );
  }
}
