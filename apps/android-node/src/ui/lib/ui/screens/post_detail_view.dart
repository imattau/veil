import 'package:flutter/material.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../components/veil_post_card.dart';
import '../theme/veil_theme.dart';

class PostDetailView extends StatefulWidget {
  final NodeEvent post;
  final SocialController controller;

  const PostDetailView({
    super.key,
    required this.post,
    required this.controller,
  });

  @override
  State<PostDetailView> createState() => _PostDetailViewState();
}

class _PostDetailViewState extends State<PostDetailView> {
  final FocusNode _replyFocusNode = FocusNode();
  NodeEvent? _replyTarget;

  void _setReplyTarget(NodeEvent? target) {
    setState(() {
      _replyTarget = target;
    });
  }

  void _startReply(NodeEvent? target) {
    _setReplyTarget(target);
    _replyFocusNode.requestFocus();
  }

  List<_ThreadedComment> _buildThreadedComments(String root) {
    final threaded = <_ThreadedComment>[];

    void collect(String parentRoot, int depth, Set<String> ancestry) {
      final comments = widget.controller.getComments(parentRoot).toList()
        ..sort((a, b) => a.seq.compareTo(b.seq));
      for (final comment in comments) {
        threaded.add(_ThreadedComment(event: comment, depth: depth));
        final childRoot = comment.objectRoot;
        if (childRoot == null ||
            childRoot.isEmpty ||
            ancestry.contains(childRoot)) {
          continue;
        }
        collect(childRoot, depth + 1, {...ancestry, childRoot});
      }
    }

    collect(root, 0, {root});
    return threaded;
  }

  @override
  void dispose() {
    _replyFocusNode.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Conversation')),
      body: ListenableBuilder(
        listenable: widget.controller,
        builder: (context, _) {
          final root = widget.post.objectRoot ?? '';
          final comments = _buildThreadedComments(root);

          return ListView.builder(
            padding: const EdgeInsets.all(16),
            itemCount: comments.length + 1,
            itemBuilder: (context, index) {
              if (index == 0) {
                return Column(
                  children: [
                    VeilPostCard(
                      event: widget.post,
                      controller: widget.controller,
                      isDetail: true,
                      onCommentTap: () => _startReply(null),
                    ),
                    const Divider(color: Colors.white10, height: 32),
                  ],
                );
              }

              final comment = comments[index - 1];
              return Padding(
                key: Key(
                  'comment-thread-${comment.event.objectRoot ?? comment.event.seq}',
                ),
                padding: EdgeInsets.only(left: comment.depth * 16.0, bottom: 8),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    VeilPostCard(
                      event: comment.event,
                      controller: widget.controller,
                      onCommentTap: () => _startReply(comment.event),
                    ),
                    TextButton.icon(
                      key: Key(
                        'reply-action-${comment.event.objectRoot ?? comment.event.seq}',
                      ),
                      onPressed: () => _startReply(comment.event),
                      icon: const Icon(
                        Icons.reply,
                        size: 16,
                        color: VeilTheme.textSecondary,
                      ),
                      label: const Text(
                        'Reply',
                        style: TextStyle(color: VeilTheme.textSecondary),
                      ),
                    ),
                  ],
                ),
              );
            },
          );
        },
      ),
      bottomNavigationBar: _ReplyBar(
        post: widget.post,
        replyTarget: _replyTarget,
        controller: widget.controller,
        focusNode: _replyFocusNode,
        onReplyTargetChanged: _setReplyTarget,
      ),
    );
  }
}

class _ReplyBar extends StatefulWidget {
  final NodeEvent post;
  final NodeEvent? replyTarget;
  final SocialController controller;
  final FocusNode focusNode;
  final ValueChanged<NodeEvent?> onReplyTargetChanged;

  const _ReplyBar({
    required this.post,
    required this.replyTarget,
    required this.controller,
    required this.focusNode,
    required this.onReplyTargetChanged,
  });

  @override
  State<_ReplyBar> createState() => _ReplyBarState();
}

class _ReplyBarState extends State<_ReplyBar> {
  final TextEditingController _textController = TextEditingController();

  String? get _targetRoot =>
      widget.replyTarget?.objectRoot ?? widget.post.objectRoot;

  @override
  void dispose() {
    _textController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final target = widget.replyTarget;
    final targetName = target == null
        ? ''
        : widget.controller.getDisplayName(target.authorPubkey ?? '');
    final targetPreview = (target?.postText ?? '').trim();

    return Container(
      padding: EdgeInsets.only(
        bottom: MediaQuery.of(context).viewInsets.bottom + 12,
        left: 16,
        right: 16,
        top: 12,
      ),
      decoration: const BoxDecoration(
        color: VeilTheme.surface,
        border: Border(top: BorderSide(color: Colors.white10)),
      ),
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          if (target != null)
            Container(
              margin: const EdgeInsets.only(bottom: 8),
              padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 8),
              decoration: BoxDecoration(
                color: Colors.white.withValues(alpha: 0.06),
                borderRadius: BorderRadius.circular(12),
                border: Border.all(color: Colors.white12),
              ),
              child: Row(
                children: [
                  const Icon(Icons.reply, size: 14, color: VeilTheme.accent),
                  const SizedBox(width: 8),
                  Expanded(
                    child: Text(
                      targetPreview.isEmpty
                          ? 'Replying to $targetName'
                          : 'Replying to $targetName: $targetPreview',
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        color: VeilTheme.textPrimary,
                        fontSize: 12,
                      ),
                    ),
                  ),
                  IconButton(
                    onPressed: () => widget.onReplyTargetChanged(null),
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
                  key: const Key('post-reply-input'),
                  controller: _textController,
                  focusNode: widget.focusNode,
                  decoration: InputDecoration(
                    hintText: target == null ? 'Post your reply' : 'Reply',
                    hintStyle: const TextStyle(color: VeilTheme.textSecondary),
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(24),
                      borderSide: BorderSide.none,
                    ),
                    fillColor: Colors.white.withValues(alpha: 0.05),
                    filled: true,
                    contentPadding: const EdgeInsets.symmetric(
                      horizontal: 16,
                      vertical: 8,
                    ),
                  ),
                ),
              ),
              const SizedBox(width: 8),
              IconButton(
                key: const Key('post-reply-send'),
                onPressed: () {
                  final text = _textController.text.trim();
                  final root = _targetRoot;
                  if (text.isNotEmpty && root != null) {
                    widget.controller.submitReply(
                      text,
                      root,
                      channelId: widget.post.channelId,
                    );
                    _textController.clear();
                    widget.onReplyTargetChanged(null);
                    widget.focusNode.unfocus();
                  }
                },
                icon: const Icon(Icons.send, color: VeilTheme.accent),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _ThreadedComment {
  final NodeEvent event;
  final int depth;

  const _ThreadedComment({required this.event, required this.depth});
}
