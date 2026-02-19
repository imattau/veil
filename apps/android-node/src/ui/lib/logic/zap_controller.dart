import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:url_launcher/url_launcher.dart';
import './node_service.dart';

class ZapController extends ChangeNotifier {
  final NodeService nodeService;
  final http.Client _client = http.Client();

  ZapController(this.nodeService);

  /// Resolves a lightning address (user@domain.com) to an LNURL pay endpoint.
  Future<String?> getInvoice(String lnAddress, int amountSats) async {
    try {
      final parts = lnAddress.split('@');
      if (parts.length != 2) return null;

      final domain = parts[1].toLowerCase().trim();
      final user = parts[0];

      // Basic SSRF protection
      if (domain == 'localhost' ||
          domain == '127.0.0.1' ||
          domain.startsWith('192.168.') ||
          domain.startsWith('10.') ||
          domain.startsWith('172.')) {
        debugPrint('Blocked potentially malicious LNURL domain: $domain');
        return null;
      }

      final url = Uri(
        scheme: 'https',
        host: domain,
        pathSegments: ['.well-known', 'lnurlp', user],
      );

      final res = await _client.get(url).timeout(const Duration(seconds: 10));
      if (res.statusCode != 200) return null;

      final metadata = jsonDecode(res.body);
      final callback = metadata['callback'] as String?;

      if (callback == null) return null;

      // Ensure callback is also HTTPS and not local
      final callbackUri = Uri.tryParse(callback);
      if (callbackUri == null ||
          callbackUri.scheme != 'https' ||
          callbackUri.host == 'localhost' ||
          callbackUri.host == '127.0.0.1') {
        return null;
      }

      final amountMsat = amountSats * 1000;
      final invoiceUri = callbackUri.replace(
        queryParameters: {
          ...callbackUri.queryParameters,
          'amount': '$amountMsat',
        },
      );
      final invoiceRes = await _client
          .get(invoiceUri)
          .timeout(const Duration(seconds: 10));
      if (invoiceRes.statusCode != 200) return null;

      final invoiceData = jsonDecode(invoiceRes.body);
      return invoiceData['pr'] as String?; // The Bolt11 invoice
    } catch (e) {
      debugPrint('LNURL error: $e');
      return null;
    }
  }

  Future<void> launchWallet(String invoice) async {
    final uri = Uri.parse('lightning:$invoice');
    if (await canLaunchUrl(uri)) {
      await launchUrl(uri);
    } else {
      throw Exception('No lightning wallet found on this device');
    }
  }

  /// Broadcasts a social proof of the zap to the Veil network.
  Future<void> broadcastZap({
    required String targetRoot,
    required int amount,
    required String authorPubkey,
    String? channelId,
  }) async {
    await nodeService.publishZap(
      namespace: 32,
      targetRoot: targetRoot,
      amount: amount,
      channelId: channelId ?? 'general',
    );
  }

  @override
  void dispose() {
    _client.close();
    super.dispose();
  }
}
