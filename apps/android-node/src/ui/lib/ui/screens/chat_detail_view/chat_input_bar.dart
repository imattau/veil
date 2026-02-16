import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../theme/veil_theme.dart';

class ChatInputBar extends StatefulWidget {
  final Future<void> Function(String) onSend;
  final String? replyTargetLabel;
  final String? replyTargetPreview;
  final VoidCallback? onClearReplyTarget;

  const ChatInputBar({
    super.key,
    required this.onSend,
    this.replyTargetLabel,
    this.replyTargetPreview,
    this.onClearReplyTarget,
  });

  @override
  State<ChatInputBar> createState() => _ChatInputBarState();
}

class _ChatInputBarState extends State<ChatInputBar> {
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
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (widget.replyTargetLabel != null)
            Container(
              margin: const EdgeInsets.only(bottom: 8),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: VeilTheme.surface,
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: Colors.white12),
              ),
              child: Row(
                children: [
                  const Icon(Icons.reply, size: 14, color: VeilTheme.accent),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      (widget.replyTargetPreview == null ||
                              widget.replyTargetPreview!.isEmpty)
                          ? 'Replying to ${widget.replyTargetLabel}'
                          : 'Replying to ${widget.replyTargetLabel}: ${widget.replyTargetPreview}',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        color: VeilTheme.textPrimary,
                        fontSize: 12,
                      ),
                    ),
                  ),
                  IconButton(
                    onPressed: widget.onClearReplyTarget,
                    icon: const Icon(
                      Icons.close,
                      size: 16,
                      color: VeilTheme.textSecondary,
                    ),
                    visualDensity: VisualDensity.compact,
                    constraints: const BoxConstraints(
                      minHeight: 28,
                      minWidth: 28,
                    ),
                  ),
                ],
              ),
            ),
          Row(
            children: [
              Expanded(
                child: TextField(
                  controller: _controller,
                  decoration: InputDecoration(
                    hintText: widget.replyTargetLabel == null
                        ? 'Message'
                        : 'Reply message',
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
        ],
      ),
    );
  }
}
