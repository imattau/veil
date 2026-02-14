import 'dart:convert';
import 'package:flutter/material.dart';
import 'package:http/http.dart' as http;
import 'package:url_launcher/url_launcher.dart';
import '../theme/veil_theme.dart';

class LinkPreviewCard extends StatefulWidget {
  final String url;

  const LinkPreviewCard({super.key, required this.url});

  @override
  State<LinkPreviewCard> createState() => _LinkPreviewCardState();
}

class _LinkPreviewCardState extends State<LinkPreviewCard> {
  late Future<_LinkMetadata?> _metadataFuture;

  @override
  void initState() {
    super.initState();
    _metadataFuture = _fetchMetadata(widget.url);
  }

  Future<_LinkMetadata?> _fetchMetadata(String url) async {
    try {
      final uri = Uri.parse(url);
      final response = await http.get(uri).timeout(const Duration(seconds: 4));
      
      if (response.statusCode >= 200 && response.statusCode < 300) {
        // Simple regex-based parsing to avoid heavy dependencies
        final html = response.body;
        
        String? title = _extractMeta(html, 'og:title') ?? 
                       _extractTag(html, 'title');
        String? description = _extractMeta(html, 'og:description') ?? 
                             _extractMeta(html, 'description');
        String? image = _extractMeta(html, 'og:image');

        if (title == null && description == null) return null;

        // Resolve relative image URLs
        if (image != null && !image.startsWith('http')) {
          image = uri.resolve(image).toString();
        }

        return _LinkMetadata(
          title: title ?? uri.host,
          description: description,
          imageUrl: image,
          url: url,
        );
      }
    } catch (_) {
      // Ignore errors, just don't show preview
    }
    return null;
  }

  String? _extractMeta(String html, String property) {
    // Look for <meta property="..." content="..."> or <meta name="..." content="...">
    final RegExp exp = RegExp(
      '<meta\\s+(?:property|name)=["\']$property["\']\\s+content=["\'](.*?)["\']',
      caseSensitive: false,
    );
    final match = exp.firstMatch(html);
    return match?.group(1);
  }

  String? _extractTag(String html, String tag) {
    final RegExp exp = RegExp(
      '<$tag.*?>(.*?)</$tag>',
      caseSensitive: false,
      dotAll: true,
    );
    final match = exp.firstMatch(html);
    return match?.group(1)?.trim();
  }

  @override
  Widget build(BuildContext context) {
    return FutureBuilder<_LinkMetadata?>(
      future: _metadataFuture,
      builder: (context, snapshot) {
        if (!snapshot.hasData || snapshot.data == null) {
          return const SizedBox.shrink();
        }

        final data = snapshot.data!;
        return GestureDetector(
          onTap: () async {
            final uri = Uri.tryParse(widget.url);
            if (uri != null) {
              await launchUrl(uri, mode: LaunchMode.externalApplication);
            }
          },
          child: Container(
            margin: const EdgeInsets.only(top: 8),
            clipBehavior: Clip.antiAlias,
            decoration: BoxDecoration(
              color: VeilTheme.surface,
              borderRadius: BorderRadius.circular(12),
              border: Border.all(color: Colors.white.withOpacity(0.1)),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (data.imageUrl != null)
                  Image.network(
                    data.imageUrl!,
                    height: 160,
                    width: double.infinity,
                    fit: BoxFit.cover,
                    errorBuilder: (_, __, ___) => const SizedBox.shrink(),
                  ),
                Padding(
                  padding: const EdgeInsets.all(12),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(
                        data.title,
                        maxLines: 1,
                        overflow: TextOverflow.ellipsis,
                        style: const TextStyle(
                          fontWeight: FontWeight.bold,
                          fontSize: 14,
                        ),
                      ),
                      if (data.description != null) ...[
                        const SizedBox(height: 4),
                        Text(
                          data.description!,
                          maxLines: 2,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                            color: VeilTheme.textSecondary,
                            fontSize: 12,
                          ),
                        ),
                      ],
                      const SizedBox(height: 8),
                      Row(
                        children: [
                          const Icon(
                            Icons.link,
                            size: 12,
                            color: VeilTheme.textSecondary,
                          ),
                          const SizedBox(width: 4),
                          Expanded(
                            child: Text(
                              Uri.parse(widget.url).host,
                              style: const TextStyle(
                                color: VeilTheme.textSecondary,
                                fontSize: 10,
                              ),
                              maxLines: 1,
                              overflow: TextOverflow.ellipsis,
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _LinkMetadata {
  final String title;
  final String? description;
  final String? imageUrl;
  final String url;

  const _LinkMetadata({
    required this.title,
    this.description,
    this.imageUrl,
    required this.url,
  });
}
