import 'package:flutter/material.dart';

import '../../theme/veil_theme.dart';

class ChatMessageBubble extends StatelessWidget {
  final String content;
  final bool isMe;
  final String senderLabel;
  final String? replySenderLabel;
  final String? replyPreview;
  final VoidCallback? onLongPress;
  final DateTime? time;

  const ChatMessageBubble({
    super.key,
    required this.content,
    required this.isMe,
    required this.senderLabel,
    this.replySenderLabel,
    this.replyPreview,
    this.onLongPress,
    this.time,
  });

  @override
  Widget build(BuildContext context) {
    return GestureDetector(
      onLongPress: onLongPress,
      behavior: HitTestBehavior.opaque,
      child: Align(
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
              if (replyPreview != null) ...[
                Container(
                  width: double.infinity,
                  margin: const EdgeInsets.only(bottom: 6),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 8,
                    vertical: 6,
                  ),
                  decoration: BoxDecoration(
                    color: isMe ? Colors.black12 : Colors.white10,
                    borderRadius: BorderRadius.circular(10),
                    border: Border.all(
                      color: isMe
                          ? Colors.black.withValues(alpha: 0.15)
                          : Colors.white12,
                    ),
                  ),
                  child: Column(
                    crossAxisAlignment: isMe
                        ? CrossAxisAlignment.end
                        : CrossAxisAlignment.start,
                    children: [
                      Text(
                        'Replying to ${replySenderLabel ?? 'message'}',
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 10,
                          fontWeight: FontWeight.w600,
                          color: isMe
                              ? Colors.black.withValues(alpha: 0.65)
                              : VeilTheme.textSecondary,
                        ),
                      ),
                      const SizedBox(height: 2),
                      Text(
                        replyPreview!,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: TextStyle(
                          fontSize: 12,
                          color: isMe
                              ? Colors.black.withValues(alpha: 0.72)
                              : VeilTheme.textPrimary,
                        ),
                      ),
                    ],
                  ),
                ),
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
                        ? Colors.black.withValues(alpha: 0.5)
                        : VeilTheme.textSecondary,
                  ),
                ),
              ],
            ],
          ),
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
