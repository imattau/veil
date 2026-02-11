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
  final TextEditingController _textController = TextEditingController();
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
    final image = await _picker.pickImage(source: ImageSource.gallery, maxWidth: 1024);
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
      }

      await widget.service.publishPost(
        text: text,
        channelId: _selectedChannel,
        // TODO: Pass mediaRoot to publishPost if implemented in node_service
      );
      if (mounted) Navigator.pop(context);
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Failed to publish: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _isPublishing = false);
    }
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
                        child: Icon(Icons.person, color: VeilTheme.textSecondary),
                      ),
                      const SizedBox(width: 12),
                      Container(
                        padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 4),
                        decoration: BoxDecoration(
                          border: Border.all(color: VeilTheme.accent.withOpacity(0.5)),
                          borderRadius: BorderRadius.circular(16),
                        ),
                        child: Text(
                          '#$_selectedChannel',
                          style: const TextStyle(
                            color: VeilTheme.accent,
                            fontSize: 12,
                            fontWeight: FontWeight.bold,
                          ),
                        ),
                      ),
                    ],
                  ),
                  TextField(
                    controller: _textController,
                    autofocus: true,
                    maxLines: null,
                    style: const TextStyle(fontSize: 18),
                    decoration: const InputDecoration(
                      hintText: "What's happening?",
                      hintStyle: TextStyle(color: VeilTheme.textSecondary),
                      border: InputBorder.none,
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
                              icon: const Icon(Icons.close, color: Colors.white),
                              onPressed: () => setState(() => _selectedImage = null),
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
          IconButton(
            onPressed: _isPublishing ? null : _pickImage,
            icon: const Icon(Icons.image_outlined, color: VeilTheme.accent),
          ),
          IconButton(
            onPressed: () {},
            icon: const Icon(Icons.poll_outlined, color: VeilTheme.accent),
          ),
          const Spacer(),
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
                ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
                : const Text('Post', style: TextStyle(fontWeight: FontWeight.bold)),
          ),
        ],
      ),
    );
  }
}
