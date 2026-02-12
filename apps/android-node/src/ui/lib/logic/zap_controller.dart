import 'dart:convert';
import 'package:flutter/foundation.dart';
import 'package:http/http.dart' as http;
import 'package:url_launcher/url_launcher.dart';
import './node_service.dart';
import './models/node_event.dart';

class ZapController extends ChangeNotifier {
  final NodeService nodeService;
  final http.Client _client = http.Client();

  ZapController(this.nodeService);

  /// Resolves a lightning address (user@domain.com) to an LNURL pay endpoint.
  Future<String?> getInvoice(String lnAddress, int amountSats) async {
    try {
      final parts = lnAddress.split('@');
      if (parts.length != 2) return null;

      final domain = parts[1];
      final user = parts[0];
      final url = 'https://$domain/.well-known/lnurlp/$user';

      final res = await _client.get(Uri.parse(url));
      final metadata = jsonDecode(res.body);
      final callback = metadata['callback'] as String?;

      if (callback == null) return null;

      final amountMsat = amountSats * 1000;
      final invoiceRes = await _client.get(
        Uri.parse('$callback?amount=$amountMsat'),
      );
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
      throw 'No lightning wallet found';
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
