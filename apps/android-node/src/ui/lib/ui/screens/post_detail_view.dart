import 'package:flutter/material.dart';
import '../../logic/models/node_event.dart';
import '../../logic/social_controller.dart';
import '../components/veil_post_card.dart';
import '../theme/veil_theme.dart';

class PostDetailView extends StatelessWidget {
  final NodeEvent post;
  final SocialController controller;

  const PostDetailView({
    super.key,
    required this.post,
    required this.controller,
  });

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Conversation'),
      ),
      body: ListenableBuilder(
        listenable: controller,
        builder: (context, _) {
          final comments = controller.getComments(post.objectRoot ?? '');
          
          return ListView.builder(
            padding: const EdgeInsets.all(16),
            itemCount: comments.length + 1,
            itemBuilder: (context, index) {
              if (index == 0) {
                return Column(
                  children: [
                    VeilPostCard(event: post, controller: controller, isDetail: true),
                    const Divider(color: Colors.white10, height: 32),
                  ],
                );
              }
              
              final comment = comments[index - 1];
              return VeilPostCard(event: comment, controller: controller);
            },
          );
        },
      ),
      bottomNavigationBar: _ReplyBar(post: post, controller: controller),
    );
  }
}

class _ReplyBar extends StatefulWidget {
  final NodeEvent post;
  final SocialController controller;

  const _ReplyBar({required this.post, required this.controller});

  @override
  State<_ReplyBar> createState() => _ReplyBarState();
}

class _ReplyBarState extends State<_ReplyBar> {
  final TextEditingController _textController = TextEditingController();

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
        color: VeilTheme.surface,
        border: Border(top: BorderSide(color: Colors.white10)),
      ),
      child: Row(
        children: [
          Expanded(
            child: TextField(
              controller: _textController,
              decoration: InputDecoration(
                hintText: 'Post your reply',
                hintStyle: const TextStyle(color: VeilTheme.textSecondary),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(24),
                  borderSide: BorderSide.none,
                ),
                fillColor: Colors.white.withOpacity(0.05),
                filled: true,
                contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
              ),
            ),
          ),
          const SizedBox(width: 8),
          IconButton(
            onPressed: () {
              final text = _textController.text.trim();
              final root = widget.post.objectRoot;
              if (text.isNotEmpty && root != null) {
                widget.controller.submitReply(text, root, channelId: widget.post.channelId);
                _textController.clear();
                FocusScope.of(context).unfocus();
              }
            },
            icon: const Icon(Icons.send, color: VeilTheme.accent),
          ),
        ],
      ),
    );
  }
}
