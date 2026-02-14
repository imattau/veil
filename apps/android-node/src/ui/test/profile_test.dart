import 'package:flutter_test/flutter_test.dart';
import 'package:veil_social/logic/models/node_event.dart';
import 'package:veil_social/logic/node_service.dart';

void main() {
  group('Profile Logic', () {
    test('NodeService updates local cache after publishProfile', () async {
      final service = NodeService();
      service.testSetIdentity('my_pubkey');

      // Initial state: no profile
      expect(service.profiles['my_pubkey'], isNull);

      // We'll use testInjectEvent to simulate the effect of publishProfile's loopback
      // or just check if the caching logic we added works via a simulated event.
      service.testInjectEvent({
        'seq': 100,
        'event': 'feed_bundle',
        'data': {
          'kind': 'profile',
          'author_pubkey_hex': 'my_pubkey',
          'display_name': 'New Name',
          'bio': 'My Bio',
        }
      });

      final profile = service.profiles['my_pubkey'];
      expect(profile, isNotNull);
      expect(profile!.displayName, 'New Name');
      expect(profile.bio, 'My Bio');
    });

    test('NodeService respects higher sequence numbers for profiles', () {
      final service = NodeService();
      service.testSetIdentity('user1');

      service.testInjectEvent({
        'seq': 1,
        'event': 'feed_bundle',
        'data': {
          'kind': 'profile',
          'author_pubkey_hex': 'user1',
          'display_name': 'Old Name',
        }
      });

      // Inject a newer profile version
      service.testInjectEvent({
        'seq': 2,
        'event': 'feed_bundle',
        'data': {
          'kind': 'profile',
          'author_pubkey_hex': 'user1',
          'display_name': 'New Name',
        }
      });

      expect(service.profiles['user1']!.displayName, 'New Name');

      // Inject an older one (should be ignored)
      service.testInjectEvent({
        'seq': 0,
        'event': 'feed_bundle',
        'data': {
          'kind': 'profile',
          'author_pubkey_hex': 'user1',
          'display_name': 'Stale Name',
        }
      });

      expect(service.profiles['user1']!.displayName, 'New Name');
    });
  });
}
