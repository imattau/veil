import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/node_service.dart';
import 'package:veil_social/logic/social_controller.dart';

void main() {
  group('Zap Logic', () {
    test('aggregates zap totals for a post', () {
      final service = NodeService();
      final controller = SocialController(service);

      // Inject a post
      service.testInjectEvent({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {
          'kind': 'post',
          'object_root': 'post123',
          'text': 'Support me!',
        }
      });

      // Inject two zaps
      service.testInjectEvent({
        'seq': 2,
        'event': 'feed_bundle',
        'data': {
          'kind': 'zap',
          'target_root': 'post123',
          'amount': 50,
        }
      });

      service.testInjectEvent({
        'seq': 3,
        'event': 'feed_bundle',
        'data': {
          'kind': 'zap',
          'target_root': 'post123',
          'amount': 100,
        }
      });

      expect(controller.getZapTotal('post123'), 150);
    });

    test('profile exposes lightning address', () {
      final event = NodeEvent.fromJson({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {
          'kind': 'profile',
          'lightning_address': 'alice@veil.io',
        }
      });

      expect(event.lightningAddress, 'alice@veil.io');
    });
  });
}
