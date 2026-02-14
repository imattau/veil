import 'dart:typed_data';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:image_picker/image_picker.dart';
import '../../logic/node_service.dart';
import '../theme/veil_theme.dart';

class ComposerView extends StatefulWidget {
  final NodeService service;
  final String? initialChannel;

  const ComposerView({super.key, required this.service, this.initialChannel});

  @override
  State<ComposerView> createState() => _ComposerViewState();
}

class _ComposerViewState extends State<ComposerView> {
  final TextEditingController _textController = SocialTextEditingController();
  String _selectedChannel = 'general';
  bool _isPublishing = false;
  Uint8List? _selectedImage;
  final ImagePicker _picker = ImagePicker();

  @override
  void initState() {
    super.initState();
    if (widget.initialChannel != null) {
      _selectedChannel = widget.initialChannel!;
    }
  }

  Future<void> _pickImage() async {
    final image = await _picker.pickImage(
      source: ImageSource.gallery,
      maxWidth: 1024,
      imageQuality: 80,
    );
    if (image != null) {
      final bytes = await image.readAsBytes();
      setState(() => _selectedImage = bytes);
    }
  }

  Future<void> _handlePublish() async {
    final text = _textController.text.trim();
    if (text.isEmpty && _selectedImage == null) return;

    setState(() => _isPublishing = true);
    try {
      String? mediaRoot;
      if (_selectedImage != null) {
        mediaRoot = await widget.service.uploadMedia(_selectedImage!);
        if (mediaRoot == null) {
          throw Exception(
            widget.service.state.lastError ?? 'Image upload failed',
          );
        }
      }

      final ok = await widget.service.publishPost(
        text: text,
        channelId: _selectedChannel,
        mediaRoots: mediaRoot != null ? [mediaRoot] : const [],
      );
      if (ok) {
        if (mounted) Navigator.pop(context);
      } else {
        throw Exception(widget.service.state.lastError ?? 'Publish failed');
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('Failed to publish: $e')));
      }
    } finally {
      if (mounted) setState(() => _isPublishing = false);
    }
  }

  Future<void> _handleCreatePoll() async {
    final result = await showDialog<_PollDraft>(
      context: context,
      builder: (context) => const _CreatePollDialog(),
    );
    if (result == null) return;
    setState(() => _isPublishing = true);
    try {
      final ok = await widget.service.publishPoll(
        question: result.question,
        options: result.options,
        channelId: _selectedChannel,
      );
      if (ok) {
        if (mounted) Navigator.pop(context);
      } else {
        throw Exception(widget.service.state.lastError ?? 'Poll failed');
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(
          context,
        ).showSnackBar(SnackBar(content: Text('Failed to publish poll: $e')));
      }
    } finally {
      if (mounted) setState(() => _isPublishing = false);
    }
  }

  void _showChannelSelector() {
    final subs = widget.service.state.subscriptions;
    showModalBottomSheet(
      context: context,
      backgroundColor: VeilTheme.surface,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(20)),
      ),
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Padding(
              padding: EdgeInsets.fromLTRB(20, 20, 20, 10),
              child: Text(
                'Select Channel',
                style: TextStyle(fontSize: 18, fontWeight: FontWeight.bold),
              ),
            ),
            Flexible(
              child: ListView.builder(
                shrinkWrap: true,
                itemCount: subs.length,
                itemBuilder: (context, index) {
                  final channel = subs[index];
                  return ListTile(
                    title: Text('#$channel'),
                    trailing: _selectedChannel == channel
                        ? const Icon(Icons.check, color: VeilTheme.accent)
                        : null,
                    onTap: () {
                      setState(() => _selectedChannel = channel);
                      Navigator.pop(context);
                    },
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: VeilTheme.background,
      appBar: AppBar(
        leading: IconButton(
          icon: const Icon(Icons.close),
          onPressed: () => Navigator.pop(context),
        ),
        title: const Text('New Post'),
      ),
      body: Column(
        children: [
          Expanded(
            child: SingleChildScrollView(
              padding: const EdgeInsets.all(16),
              child: Column(
                children: [
                  Row(
                    children: [
                      const CircleAvatar(
                        backgroundColor: VeilTheme.surface,
                        child: Icon(
                          Icons.person,
                          color: VeilTheme.textSecondary,
                        ),
                      ),
                      const SizedBox(width: 12),
                      InkWell(
                        onTap: _showChannelSelector,
                        borderRadius: BorderRadius.circular(16),
                        child: Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 4,
                          ),
                          decoration: BoxDecoration(
                            border: Border.all(
                              color: VeilTheme.accent.withOpacity(0.5),
                            ),
                            borderRadius: BorderRadius.circular(16),
                          ),
                          child: Row(
                            mainAxisSize: MainAxisSize.min,
                            children: [
                              Text(
                                '#$_selectedChannel',
                                style: const TextStyle(
                                  color: VeilTheme.accent,
                                  fontSize: 12,
                                  fontWeight: FontWeight.bold,
                                ),
                              ),
                              const SizedBox(width: 4),
                              const Icon(
                                Icons.keyboard_arrow_down,
                                size: 14,
                                color: VeilTheme.accent,
                              ),
                            ],
                          ),
                        ),
                      ),
                    ],
                  ),
                  TextField(
                    controller: _textController,
                    autofocus: true,
                    maxLines: null,
                    maxLength: 4096,
                    style: const TextStyle(fontSize: 18),
                    decoration: const InputDecoration(
                      hintText: "What's happening?",
                      hintStyle: TextStyle(color: VeilTheme.textSecondary),
                      border: InputBorder.none,
                      counterText: '',
                    ),
                  ),
                  if (_selectedImage != null)
                    Stack(
                      key: const ValueKey('image_preview'),
                      children: [
                        Container(
                          height: 200,
                          width: double.infinity,
                          decoration: BoxDecoration(
                            borderRadius: BorderRadius.circular(12),
                            image: DecorationImage(
                              image: MemoryImage(_selectedImage!),
                              fit: BoxFit.cover,
                            ),
                          ),
                        ),
                        Positioned(
                          right: 8,
                          top: 8,
                          child: CircleAvatar(
                            backgroundColor: Colors.black54,
                            child: IconButton(
                              icon: const Icon(
                                Icons.close,
                                color: Colors.white,
                              ),
                              onPressed: () =>
                                  setState(() => _selectedImage = null),
                            ),
                          ),
                        ),
                      ],
                    ),
                ],
              ),
            ),
          ),
          _buildBottomActionToolbar(),
        ],
      ),
    );
  }

  Widget _buildBottomActionToolbar() {
    return Container(
      padding: const EdgeInsets.only(bottom: 12, left: 16, right: 16, top: 12),
      decoration: const BoxDecoration(
        color: VeilTheme.surface,
        border: Border(top: BorderSide(color: Colors.white10)),
      ),
      child: Row(
        children: [
          IconButton(
            onPressed: _isPublishing ? null : _pickImage,
            icon: const Icon(Icons.image_outlined, color: VeilTheme.accent),
          ),
          IconButton(
            onPressed: _isPublishing ? null : _handleCreatePoll,
            icon: const Icon(Icons.poll_outlined, color: VeilTheme.accent),
          ),
          const Spacer(),
          ListenableBuilder(
            listenable: _textController,
            builder: (context, _) {
              final count = _textController.text.length;
              return Text(
                '$count / 4096',
                style: TextStyle(
                  color: count > 4000 ? Colors.red : VeilTheme.textSecondary,
                  fontSize: 12,
                ),
              );
            },
          ),
          const SizedBox(width: 16),
          ElevatedButton(
            onPressed: _isPublishing ? null : _handlePublish,
            style: ElevatedButton.styleFrom(
              backgroundColor: VeilTheme.accent,
              foregroundColor: Colors.black,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(24),
              ),
              padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
              elevation: 0,
            ),
            child: _isPublishing
                ? const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text(
                    'Post',
                    style: TextStyle(fontWeight: FontWeight.bold),
                  ),
          ),
        ],
      ),
    );
  }
}

class _PollDraft {
  final String question;
  final List<String> options;

  const _PollDraft({required this.question, required this.options});
}

class _CreatePollDialog extends StatefulWidget {
  const _CreatePollDialog();

  @override
  State<_CreatePollDialog> createState() => _CreatePollDialogState();
}

class _CreatePollDialogState extends State<_CreatePollDialog> {
  final TextEditingController _questionController = TextEditingController();
  final List<TextEditingController> _optionControllers = [
    TextEditingController(),
    TextEditingController(),
  ];

  @override
  void dispose() {
    _questionController.dispose();
    for (final controller in _optionControllers) {
      controller.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Create Poll'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: _questionController,
              decoration: const InputDecoration(labelText: 'Question'),
            ),
            const SizedBox(height: 12),
            ..._optionControllers.asMap().entries.map((entry) {
              final index = entry.key;
              final controller = entry.value;
              return Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: TextField(
                  controller: controller,
                  decoration: InputDecoration(labelText: 'Option ${index + 1}'),
                ),
              );
            }),
            if (_optionControllers.length < 4)
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () {
                    setState(() {
                      _optionControllers.add(TextEditingController());
                    });
                  },
                  icon: const Icon(Icons.add),
                  label: const Text('Add option'),
                ),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        TextButton(
          onPressed: () {
            final question = _questionController.text.trim();
            final options = _optionControllers
                .map((controller) => controller.text.trim())
                .where((text) => text.isNotEmpty)
                .toList();
            if (question.isEmpty || options.length < 2) {
              ScaffoldMessenger.of(context).showSnackBar(
                const SnackBar(
                  content: Text('Provide a question and at least 2 options'),
                ),
              );
              return;
            }
            Navigator.pop(
              context,
              _PollDraft(question: question, options: options),
            );
          },
          child: const Text('Create'),
        ),
      ],
    );
  }
}

class SocialTextEditingController extends TextEditingController {
  @override
  TextSpan buildTextSpan({
    required BuildContext context,
    TextStyle? style,
    required bool withComposing,
  }) {
    final text = value.text;
    final List<InlineSpan> spans = [];
    final pattern = RegExp(
      r'(https?:\/\/[^\s]+|#[A-Za-z0-9_]+|@[A-Za-z0-9_]+)',
    );

    var cursor = 0;
    for (final match in pattern.allMatches(text)) {
      if (match.start > cursor) {
        spans.add(
          TextSpan(text: text.substring(cursor, match.start), style: style),
        );
      }

      final token = match.group(0)!;
      spans.add(
        TextSpan(
          text: token,
          style: style?.copyWith(
            color: VeilTheme.accent,
            fontWeight: FontWeight.bold,
          ),
        ),
      );

      cursor = match.end;
    }

    if (cursor < text.length) {
      spans.add(TextSpan(text: text.substring(cursor), style: style));
    }

    return TextSpan(style: style, children: spans);
  }
}
