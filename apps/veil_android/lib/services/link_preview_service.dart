import 'package:http/http.dart' as http;

import '../models.dart';

class LinkPreviewService {
  final Map<String, LinkPreview> _cache = {};
  final RegExp _urlRegex = RegExp(r'(https?://[^\s]+)', caseSensitive: false);

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
