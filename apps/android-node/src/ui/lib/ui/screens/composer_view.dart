import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:image_picker/image_picker.dart';
import '../../logic/node_service.dart';
import '../theme/veil_theme.dart';

part 'composer_view/composer_view_poll_dialog.dart';
part 'composer_view/composer_view_social_text_controller.dart';

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

  bool get _serviceReady => widget.service.state.running;

  @override
  void initState() {
    super.initState();
    _textController.addListener(_handleDraftChanged);
    if (widget.initialChannel != null) {
      final initial = widget.initialChannel!.trim().replaceFirst(
        RegExp(r'^#'),
        '',
      );
      if (initial.isNotEmpty) {
        _selectedChannel = initial;
      }
    }
  }

  @override
  void dispose() {
    _textController.removeListener(_handleDraftChanged);
    _textController.dispose();
    super.dispose();
  }

  void _handleDraftChanged() {
    if (!mounted) return;
    setState(() {});
  }

  Future<void> _pickImage() async {
    if (!_serviceReady) {
      _showNotReadyMessage('attach media');
      return;
    }
    final image = await _picker.pickImage(
      source: ImageSource.gallery,
      maxWidth: 2048,
      maxHeight: 2048,
      imageQuality: 88,
    );
    if (image != null) {
      final bytes = await image.readAsBytes();
      setState(() => _selectedImage = bytes);
    }
  }

  Future<void> _handlePublish() async {
    if (!_serviceReady) {
      _showNotReadyMessage('publish');
      return;
    }
    if (!_hasDraft) return;
    final text = _textController.text.trim();

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
    if (!_serviceReady) {
      _showNotReadyMessage('publish a poll');
      return;
    }
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

  void _showNotReadyMessage(String action) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(content: Text('Node is still starting; cannot $action yet.')),
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
                        borderRadius: BorderRadius.circular(VeilTheme.radiusM),
                        child: Container(
                          padding: const EdgeInsets.symmetric(
                            horizontal: 12,
                            vertical: 4,
                          ),
                          decoration: BoxDecoration(
                            border: Border.all(
                              color: VeilTheme.accent.withValues(alpha: 0.5),
                            ),
                            borderRadius: BorderRadius.circular(
                              VeilTheme.radiusM,
                            ),
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
                            borderRadius: BorderRadius.circular(
                              VeilTheme.radiusS,
                            ),
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
    final canUseComposerTools = _serviceReady && !_isPublishing;
    final canPublish = canUseComposerTools && _hasDraft;
    return Container(
      padding: const EdgeInsets.only(bottom: 12, left: 16, right: 16, top: 12),
      decoration: const BoxDecoration(
        color: VeilTheme.surface,
        border: Border(top: BorderSide(color: Colors.white10)),
      ),
      child: Row(
        children: [
          IconButton(
            tooltip: 'Attach image',
            onPressed: canUseComposerTools ? _pickImage : null,
            icon: const Icon(Icons.image_outlined, color: VeilTheme.accent),
          ),
          IconButton(
            tooltip: 'Create poll',
            onPressed: canUseComposerTools ? _handleCreatePoll : null,
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
            onPressed: canPublish ? _handlePublish : null,
            style: ElevatedButton.styleFrom(
              backgroundColor: VeilTheme.accent,
              foregroundColor: Colors.black,
              disabledBackgroundColor: VeilTheme.accent.withValues(alpha: 0.25),
              disabledForegroundColor: Colors.black54,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(VeilTheme.radiusL),
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

  bool get _hasDraft =>
      _textController.text.trim().isNotEmpty || _selectedImage != null;
}
