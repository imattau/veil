import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';
import 'package:mime/mime.dart';
import 'package:veil_sdk/veil_sdk.dart';

import '../models.dart';
class ComposeScreen extends StatefulWidget {
  final void Function(String text, List<Attachment> attachments) onPublish;
  final String channelLabel;

  const ComposeScreen({
    super.key,
    required this.onPublish,
    required this.channelLabel,
  });

  @override
  State<ComposeScreen> createState() => _ComposeScreenState();
}

class _ComposeScreenState extends State<ComposeScreen> {
  final _controller = TextEditingController();
  final _picker = ImagePicker();
  final List<Attachment> _attachments = [];
  bool _attachMenuOpen = false;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Compose'),
        actions: [
          TextButton(
            onPressed: () => widget.onPublish(_controller.text, _attachments),
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
                    'Posting to ${widget.channelLabel}',
                    style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: Colors.white70,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 12),
              TextField(
                controller: _controller,
                maxLines: 8,
                autofocus: true,
                decoration: InputDecoration(
                  hintText: 'Share an update...',
                  suffixIcon: IconButton(
                    tooltip: 'Attach',
                    icon: const Icon(Icons.attach_file),
                    onPressed: _openAttachMenu,
                  ),
                ),
              ),
              const SizedBox(height: 16),
              Text(
                '${_attachments.length} attached',
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                  color: Colors.white70,
                ),
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
    final chunks = splitIntoFileChunks(bytes);
    final chunkRoots = chunks
        .map((chunk) => sha256.convert(chunk.data).toString())
        .toList();
    final descriptor = MediaDescriptorV1(
      mime: image.mimeType ?? 'image/*',
      size: bytes.length,
      hashHex: digest,
      chunkRoots: chunkRoots,
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
          chunkCount: chunks.length,
          descriptor: descriptor,
        ),
      );
    });
  }

  Future<void> _pickFile() async {
    final result = await FilePicker.platform.pickFiles(withData: true);
    if (result == null || result.files.isEmpty) return;
    final file = result.files.first;
    final bytes = file.bytes;
    if (bytes == null) return;
    final mime = lookupMimeType(file.name) ?? 'application/octet-stream';
    final digest = sha256.convert(bytes).toString();
    final chunks = splitIntoFileChunks(bytes);
    final chunkRoots = chunks
        .map((chunk) => sha256.convert(chunk.data).toString())
        .toList();
    final descriptor = MediaDescriptorV1(
      mime: mime,
      size: bytes.length,
      hashHex: digest,
      chunkRoots: chunkRoots,
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
          chunkCount: chunks.length,
          descriptor: descriptor,
        ),
      );
    });
  }
}
