@TestOn("vm")
import "dart:io";

import "package:flutter_reactive_ble/flutter_reactive_ble.dart";
import "package:flutter_test/flutter_test.dart";
import "package:veil_sdk/src/lanes/ble_lane.dart";
import "package:veil_sdk/src/lanes/tor_lane.dart";

void main() {
  testWidgets("p2p multilane harness (ble)", (tester) async {
    final deviceId = Platform.environment["VEIL_TEST_BLE_DEVICE"];
    if (deviceId == null || deviceId.isEmpty) {
      return;
    }
    final ble = FlutterReactiveBle();
    final lane = BleLane(
      ble: ble,
      deviceId: deviceId,
      serviceUuid: Uuid.parse("0000180f-0000-1000-8000-00805f9b34fb"),
      characteristicUuid: Uuid.parse("00002a19-0000-1000-8000-00805f9b34fb"),
    );
    await lane.send(deviceId, [1, 2, 3]);
    await lane.close();
  }, skip: Platform.environment["VEIL_TEST_BLE_DEVICE"] == null);

  testWidgets("p2p multilane harness (tor)", (tester) async {
    final url = Platform.environment["VEIL_TEST_TOR_URL"];
    if (url == null || url.isEmpty) {
      return;
    }
    final lane = TorLane(url: url);
    await lane.send("tor", [7, 7, 7]);
    await lane.close();
  }, skip: Platform.environment["VEIL_TEST_TOR_URL"] == null);
}
