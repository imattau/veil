import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';
import 'package:mime/mime.dart';
import 'package:veil_sdk/veil_sdk.dart';

import '../app_controller.dart';
import '../models.dart';
class ComposeScreen extends StatefulWidget {
  final void Function(
    String text,
    List<Attachment> attachments,
    String channelLabel,
  ) onPublish;
  final String channelLabel;
  final VeilAppController controller;
  final VoidCallback onManageChannels;

  const ComposeScreen({
    super.key,
    required this.onPublish,
    required this.channelLabel,
    required this.controller,
    required this.onManageChannels,
  });

  @override
  State<ComposeScreen> createState() => _ComposeScreenState();
}

class _ComposeScreenState extends State<ComposeScreen> {
  final _controller = TextEditingController();
  final _channelController = TextEditingController();
  final _picker = ImagePicker();
  final List<Attachment> _attachments = [];
  bool _attachMenuOpen = false;
  bool _processingAttachment = false;

  @override
  void initState() {
    super.initState();
    _channelController.text = widget.channelLabel;
  }

  @override
  void dispose() {
    _controller.dispose();
    _channelController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Compose'),
        actions: [
          TextButton(
            onPressed: _processingAttachment
                ? null
                : () => widget.onPublish(
                      _controller.text,
                      _attachments,
                      _channelController.text.trim(),
                    ),
            child: const Text('Publish'),
          ),
        ],
      ),
      body: SafeArea(
        child: SingleChildScrollView(
          padding: EdgeInsets.only(
            left: 16,
            right: 16,
            top: 16,
            bottom: 16 + MediaQuery.of(context).viewInsets.bottom,
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  const Icon(Icons.tag, size: 16, color: Color(0xFF60A5FA)),
                  const SizedBox(width: 6),
                  Text(
                    'Posting to',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Colors.white70,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 6),
              ChannelPicker(
                controller: widget.controller,
                value: _channelController.text.trim().isEmpty
                    ? widget.channelLabel
                    : _channelController.text.trim(),
                onChanged: (value) {
                  setState(() {
                    _channelController.text = value;
                  });
                },
                onManage: widget.onManageChannels,
              ),
              const SizedBox(height: 12),
              Row(
                children: [
                  IconButton(
                    onPressed: _pickImage,
                    icon: const Icon(Icons.image_outlined),
                    tooltip: 'Attach Image',
                  ),
                  IconButton(
                    onPressed: _pickFile,
                    icon: const Icon(Icons.insert_drive_file_outlined),
                    tooltip: 'Attach File',
                  ),
                  const Spacer(),
                ],
              ),
              const SizedBox(height: 8),
              TextField(
                controller: _controller,
                maxLines: 8,
                autofocus: true,
                decoration: const InputDecoration(
                  hintText: 'Share an update...',
                ),
              ),
              const SizedBox(height: 16),
              Row(
                children: [
                  Text(
                    '${_attachments.length} attached',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Colors.white70,
                    ),
                  ),
                  if (_processingAttachment) ...[
                    const SizedBox(width: 12),
                    const SizedBox(
                      width: 14,
                      height: 14,
                      child: CircularProgressIndicator(strokeWidth: 2),
                    ),
                    const SizedBox(width: 6),
                    Text(
                      'Processing…',
                      style: Theme.of(context)
                          .textTheme
                          .bodySmall
                          ?.copyWith(color: Colors.white70),
                    ),
                  ],
                ],
              ),
              if (_attachments.isNotEmpty) ...[
                const SizedBox(height: 12),
                SizedBox(
                  height: 120,
                  child: ListView.separated(
                    scrollDirection: Axis.horizontal,
                    itemCount: _attachments.length,
                    separatorBuilder: (_, __) => const SizedBox(width: 12),
                    itemBuilder: (context, index) {
                      final attachment = _attachments[index];
                      return Stack(
                        children: [
                          ClipRRect(
                            borderRadius: BorderRadius.circular(12),
                            child: attachment.isImage
                                ? Image.memory(
                                    attachment.bytes,
                                    width: 120,
                                    height: 120,
                                    fit: BoxFit.cover,
                                  )
                                : Container(
                                    width: 120,
                                    height: 120,
                                    color: const Color(0xFF0F172A),
                                    child: Column(
                                      mainAxisAlignment: MainAxisAlignment.center,
                                      children: [
                                        const Icon(Icons.insert_drive_file),
                                        const SizedBox(height: 6),
                                        Text(
                                          attachment.name,
                                          maxLines: 2,
                                          overflow: TextOverflow.ellipsis,
                                          textAlign: TextAlign.center,
                                          style: Theme.of(context)
                                              .textTheme
                                              .bodySmall,
                                        ),
                                      ],
                                    ),
                                  ),
                          ),
                          Positioned(
                            right: 6,
                            top: 6,
                            child: InkWell(
                              onTap: () {
                                setState(() => _attachments.removeAt(index));
                              },
                              child: Container(
                                width: 28,
                                height: 28,
                                decoration: BoxDecoration(
                                  color: Colors.black54,
                                  borderRadius: BorderRadius.circular(14),
                                ),
                                child: const Icon(Icons.close, size: 16),
                              ),
                            ),
                          ),
                        ],
                      );
                    },
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }

  Future<void> _openAttachMenu() async {
    if (_attachMenuOpen) return;
    _attachMenuOpen = true;
    if (!mounted) {
      _attachMenuOpen = false;
      return;
    }
    await showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      builder: (context) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.image_outlined),
              title: const Text('Attach image'),
              onTap: () async {
                Navigator.of(context).pop();
                await _pickImage();
              },
            ),
            ListTile(
              leading: const Icon(Icons.insert_drive_file_outlined),
              title: const Text('Attach file'),
              onTap: () async {
                Navigator.of(context).pop();
                await _pickFile();
              },
            ),
          ],
        ),
      ),
    );
    _attachMenuOpen = false;
  }

  Future<void> _pickImage() async {
    final image = await _picker.pickImage(source: ImageSource.gallery);
    if (image == null) return;
    final bytes = await image.readAsBytes();
    final digest = sha256.convert(bytes).toString();
    setState(() => _processingAttachment = true);
    try {
      final result = await compute(_buildChunkMeta, bytes);
      final descriptor = MediaDescriptorV1(
        mime: image.mimeType ?? 'image/*',
        size: bytes.length,
        hashHex: digest,
        chunkRoots: result.chunkRoots,
      );
      setState(() {
        _attachments.add(
          Attachment(
            name: image.name,
            mime: image.mimeType ?? 'image/*',
            bytes: bytes,
            hashHex: digest,
            size: bytes.length,
            isImage: true,
            isVideo: false,
            chunkCount: result.chunkCount,
            descriptor: descriptor,
          ),
        );
      });
    } finally {
      if (mounted) {
        setState(() => _processingAttachment = false);
      }
    }
  }

  Future<void> _pickFile() async {
    final result = await FilePicker.platform.pickFiles(withData: true);
    if (result == null || result.files.isEmpty) return;
    final file = result.files.first;
    final bytes = file.bytes;
    if (bytes == null) return;
    final mime = lookupMimeType(file.name) ?? 'application/octet-stream';
    final digest = sha256.convert(bytes).toString();
    setState(() => _processingAttachment = true);
    try {
      final resultMeta = await compute(_buildChunkMeta, bytes);
      final descriptor = MediaDescriptorV1(
        mime: mime,
        size: bytes.length,
        hashHex: digest,
        chunkRoots: resultMeta.chunkRoots,
      );
      setState(() {
        _attachments.add(
          Attachment(
            name: file.name,
            mime: mime,
            bytes: bytes,
            hashHex: digest,
            size: bytes.length,
            isImage: mime.startsWith('image/'),
            isVideo: mime.startsWith('video/'),
            chunkCount: resultMeta.chunkCount,
            descriptor: descriptor,
          ),
        );
      });
    } finally {
      if (mounted) {
        setState(() => _processingAttachment = false);
      }
    }
  }
}

class ChannelPicker extends StatelessWidget {
  final VeilAppController controller;
  final String value;
  final ValueChanged<String> onChanged;
  final VoidCallback onManage;

  const ChannelPicker({
    super.key,
    required this.controller,
    required this.value,
    required this.onChanged,
    required this.onManage,
  });

  @override
  Widget build(BuildContext context) {
    final channels = controller.channels;
    final options = channels.isNotEmpty
        ? channels
        : [ChannelInfo(label: value, tagHex: '', isDefault: true)];
    return Row(
      children: [
        Expanded(
          child: DropdownButtonFormField<String>(
            value: options.any((c) => c.label == value)
                ? value
                : options.first.label,
            items: options
                .map(
                  (channel) => DropdownMenuItem(
                    value: channel.label,
                    child: Text(
                      channel.label.startsWith('tag:')
                          ? 'Custom tag'
                          : '#${channel.label}',
                    ),
                  ),
                )
                .toList(),
            onChanged: (next) {
              if (next != null) {
                onChanged(next);
              }
            },
            decoration: const InputDecoration(
              labelText: 'Posting to',
            ),
          ),
        ),
        const SizedBox(width: 8),
        IconButton(
          onPressed: onManage,
          tooltip: 'Manage channels',
          icon: const Icon(Icons.tune),
        ),
      ],
    );
  }
}

class _ChunkMetaResult {
  final List<String> chunkRoots;
  final int chunkCount;

  const _ChunkMetaResult(this.chunkRoots, this.chunkCount);
}

_ChunkMetaResult _buildChunkMeta(Uint8List bytes) {
  final chunks = splitIntoFileChunks(bytes);
  final chunkRoots = chunks
      .map((chunk) => sha256.convert(chunk.data).toString())
      .toList();
  return _ChunkMetaResult(chunkRoots, chunks.length);
}
