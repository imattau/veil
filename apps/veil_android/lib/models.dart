import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'package:url_launcher/url_launcher.dart';
import 'package:video_player/video_player.dart';
import 'package:veil_sdk/veil_sdk.dart';
class FeedEntry {
  final String id;
  final String author;
  final String body;
  final String? blurHash;
  final List<Attachment> attachments;
  final List<LinkPreview> linkPreviews;
  bool reconstructed;
  bool isGhost;
  final DateTime timestamp;
  int shardsHave;
  int shardsTotal;
  bool requestingMissing;
  bool fadedIn;

  FeedEntry({
    required this.id,
    required this.author,
    required this.body,
    this.blurHash,
    this.attachments = const [],
    List<LinkPreview> linkPreviews = const [],
    required this.reconstructed,
    required this.timestamp,
    this.isGhost = false,
    this.shardsHave = 0,
    this.shardsTotal = 16,
    this.requestingMissing = false,
    this.fadedIn = false,
  }) : linkPreviews = linkPreviews;

  factory FeedEntry.empty() => FeedEntry(
    id: 'empty',
    author: '',
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

class LinkPreviewService {
  final Map<String, LinkPreview> _cache = {};
  final RegExp _urlRegex = RegExp(r'(https?://[^\\s]+)', caseSensitive: false);

  List<LinkPreview> extractCached(String text) {
    final matches = _urlRegex.allMatches(text);
    final previews = <LinkPreview>[];
    for (final match in matches) {
      final url = match.group(0);
      if (url == null) continue;
      final preview = _cache[url];
      if (preview != null) {
        previews.add(preview);
      }
    }
    return previews;
  }

  Future<void> prefetch(String text) async {
    final matches = _urlRegex.allMatches(text);
    for (final match in matches) {
      final url = match.group(0);
      if (url == null || _cache.containsKey(url)) continue;
      final uri = Uri.tryParse(url);
      if (uri == null) continue;
      try {
        final response = await http.get(uri);
        if (response.statusCode < 200 || response.statusCode >= 300) {
          continue;
        }
        final preview = _parseOpenGraph(uri, response.body);
        if (preview != null) {
          _cache[url] = preview;
        }
      } catch (_) {}
    }
  }

  LinkPreview? _parseOpenGraph(Uri url, String html) {
    String? title;
    String? description;
    String? image;

    final metaTag = RegExp(r'<meta[^>]+>', caseSensitive: false);
    final attr = RegExp("(property|name)=[\"']([^\"']+)[\"']");
    final content = RegExp("content=[\"']([^\"']+)[\"']");
    for (final match in metaTag.allMatches(html)) {
      final tag = match.group(0) ?? '';
      final attrMatch = attr.firstMatch(tag);
      final contentMatch = content.firstMatch(tag);
      if (attrMatch == null || contentMatch == null) continue;
      final key = attrMatch.group(2)?.toLowerCase();
      final value = contentMatch.group(1);
      if (key == null || value == null) continue;
      if (key == 'og:title' && title == null) title = value;
      if (key == 'og:description' && description == null) {
        description = value;
      }
      if (key == 'og:image' && image == null) image = value;
    }

    if (title == null || title.isEmpty) {
      final titleMatch = RegExp(r'<title>([^<]+)</title>', caseSensitive: false)
          .firstMatch(html);
      title = titleMatch?.group(1)?.trim();
    }

    if (title == null || title.isEmpty) {
      return null;
    }
    return LinkPreview(
      url: url,
      title: title,
      description: description,
      imageUrl: image,
    );
  }
}

class LinkPreviewCard extends StatelessWidget {
  final LinkPreview preview;

  const LinkPreviewCard({required this.preview});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: InkWell(
        onTap: () => launchUrl(preview.url, mode: LaunchMode.externalApplication),
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
                          style: Theme.of(context)
                              .textTheme
                              .bodySmall
                              ?.copyWith(color: Colors.white70),
                        ),
                      ],
                      const SizedBox(height: 6),
                      Text(
                        preview.url.host,
                        style: Theme.of(context)
                            .textTheme
                            .bodySmall
                            ?.copyWith(color: Colors.white54),
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
    final temp = await File('${Directory.systemTemp.path}/${DateTime.now().millisecondsSinceEpoch}.mp4').create();
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
