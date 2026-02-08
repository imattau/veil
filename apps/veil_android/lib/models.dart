import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:video_player/video_player.dart';
import 'package:veil_sdk/veil_sdk.dart';

class ShardProgress {
  final int have;
  final int total;
  final bool requesting;
  final bool reconstructed;

  const ShardProgress({
    required this.have,
    required this.total,
    required this.requesting,
    required this.reconstructed,
  });

  factory ShardProgress.initial(int have, int total) => ShardProgress(
        have: have,
        total: total,
        requesting: false,
        reconstructed: false,
      );
}

class FeedEntry {
  final String id;
  final String author;
  final String authorKey;
  final String body;
  final String? blurHash;
  final List<Attachment> attachments;
  final List<LinkPreview> linkPreviews;
  bool _reconstructed;
  bool isGhost;
  final DateTime timestamp;
  int _shardsHave;
  int _shardsTotal;
  bool _requestingMissing;
  bool fadedIn;
  final ValueNotifier<ShardProgress> progressNotifier;

  FeedEntry({
    required this.id,
    required this.author,
    required this.authorKey,
    required this.body,
    this.blurHash,
    this.attachments = const [],
    List<LinkPreview> linkPreviews = const [],
    required bool reconstructed,
    required this.timestamp,
    this.isGhost = false,
    int shardsHave = 0,
    int shardsTotal = 16,
    bool requestingMissing = false,
    this.fadedIn = false,
  })  : linkPreviews = linkPreviews,
        _reconstructed = reconstructed,
        _shardsHave = shardsHave,
        _shardsTotal = shardsTotal,
        _requestingMissing = requestingMissing,
        progressNotifier = ValueNotifier(ShardProgress(
          have: shardsHave,
          total: shardsTotal,
          requesting: requestingMissing,
          reconstructed: reconstructed,
        ));

  bool get reconstructed => _reconstructed;
  int get shardsHave => _shardsHave;
  int get shardsTotal => _shardsTotal;
  bool get requestingMissing => _requestingMissing;

  set reconstructed(bool value) {
    if (_reconstructed == value) return;
    _reconstructed = value;
    _notify();
  }

  set shardsHave(int value) {
    if (_shardsHave == value) return;
    _shardsHave = value;
    _notify();
  }

  set shardsTotal(int value) {
    if (_shardsTotal == value) return;
    _shardsTotal = value;
    _notify();
  }

  set requestingMissing(bool value) {
    if (_requestingMissing == value) return;
    _requestingMissing = value;
    _notify();
  }

  void _notify() {
    progressNotifier.value = ShardProgress(
      have: _shardsHave,
      total: _shardsTotal,
      requesting: _requestingMissing,
      reconstructed: _reconstructed,
    );
  }

  factory FeedEntry.empty() => FeedEntry(
        id: 'empty',
        author: '',
        authorKey: '',
        body: '',
        reconstructed: false,
        timestamp: DateTime.now(),
      );
}

class Attachment {
  final String name;
  final String mime;
  final Uint8List bytes;
  final String hashHex;
  final int size;
  final bool isImage;
  final bool isVideo;
  final int chunkCount;
  final MediaDescriptorV1? descriptor;

  const Attachment({
    required this.name,
    required this.mime,
    required this.bytes,
    required this.hashHex,
    required this.size,
    required this.isImage,
    required this.isVideo,
    required this.chunkCount,
    this.descriptor,
  });
}

class LinkPreview {
  final Uri url;
  final String title;
  final String? description;
  final String? imageUrl;

  const LinkPreview({
    required this.url,
    required this.title,
    this.description,
    this.imageUrl,
  });
}

class PrivateMessage {
  final String id;
  final String from;
  final String to;
  final String body;
  final DateTime timestamp;
  final bool incoming;

  const PrivateMessage({
    required this.id,
    required this.from,
    required this.to,
    required this.body,
    required this.timestamp,
    required this.incoming,
  });
}

class LinkPreviewCard extends StatelessWidget {
  final LinkPreview preview;

  const LinkPreviewCard({required this.preview});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: InkWell(
        onTap: () =>
            launchUrl(preview.url, mode: LaunchMode.externalApplication),
        child: Container(
          decoration: BoxDecoration(
            color: Theme.of(context).colorScheme.surfaceContainerHighest,
            borderRadius: BorderRadius.circular(14),
            border: Border.all(color: const Color(0xFF1F2937)),
          ),
          child: Row(
            children: [
              if (preview.imageUrl != null)
                ClipRRect(
                  borderRadius: const BorderRadius.only(
                    topLeft: Radius.circular(14),
                    bottomLeft: Radius.circular(14),
                  ),
                  child: Image.network(
                    preview.imageUrl!,
                    width: 96,
                    height: 96,
                    fit: BoxFit.cover,
                    errorBuilder: (_, __, ___) => const SizedBox(
                      width: 96,
                      height: 96,
                      child: Icon(Icons.link),
                    ),
                  ),
                ),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.all(12),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        preview.title,
                        maxLines: 2,
                        overflow: TextOverflow.ellipsis,
                        style: Theme.of(context).textTheme.titleSmall,
                      ),
                      if (preview.description != null) ...[
                        const SizedBox(height: 6),
                        Text(
                          preview.description!,
                          maxLines: 3,
                          overflow: TextOverflow.ellipsis,
                          style: Theme.of(context).textTheme.bodySmall
                              ?.copyWith(color: Colors.white70),
                        ),
                      ],
                      const SizedBox(height: 6),
                      Text(
                        preview.url.host,
                        style: Theme.of(
                          context,
                        ).textTheme.bodySmall?.copyWith(color: Colors.white54),
                      ),
                    ],
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

class VideoAttachmentPreview extends StatefulWidget {
  final Uint8List bytes;
  final String title;

  const VideoAttachmentPreview({required this.bytes, required this.title});

  @override
  State<VideoAttachmentPreview> createState() => _VideoAttachmentPreviewState();
}

class _VideoAttachmentPreviewState extends State<VideoAttachmentPreview> {
  VideoPlayerController? _controller;
  bool _initialized = false;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    final temp = await File(
      '${Directory.systemTemp.path}/${DateTime.now().millisecondsSinceEpoch}.mp4',
    ).create();
    await temp.writeAsBytes(widget.bytes, flush: true);
    final controller = VideoPlayerController.file(temp);
    await controller.initialize();
    setState(() {
      _controller = controller;
      _initialized = true;
    });
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!_initialized || _controller == null) {
      return Container(
        color: const Color(0xFF0F172A),
        child: Center(
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              const Icon(Icons.play_circle_outline, size: 32),
              const SizedBox(height: 6),
              Text(
                widget.title,
                maxLines: 2,
                overflow: TextOverflow.ellipsis,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
      );
    }
    return Stack(
      children: [
        AspectRatio(
          aspectRatio: _controller!.value.aspectRatio,
          child: VideoPlayer(_controller!),
        ),
        Align(
          alignment: Alignment.center,
          child: IconButton(
            icon: Icon(
              _controller!.value.isPlaying ? Icons.pause : Icons.play_arrow,
              color: Colors.white,
            ),
            onPressed: () {
              setState(() {
                if (_controller!.value.isPlaying) {
                  _controller!.pause();
                } else {
                  _controller!.play();
                }
              });
            },
          ),
        ),
      ],
    );
  }
}
