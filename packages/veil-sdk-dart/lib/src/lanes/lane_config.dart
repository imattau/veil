import "lane.dart";
import "multi_lane.dart";

class LaneConfigResult {
  final VeilLane fastLane;
  final VeilLane? fallbackLane;
  final VeilLane wsLane;
  final VeilLane? quicLane;
  final List<VeilLane> p2pLanes;
  final VeilLane publishLane;

  const LaneConfigResult({
    required this.fastLane,
    required this.fallbackLane,
    required this.wsLane,
    required this.quicLane,
    required this.p2pLanes,
    required this.publishLane,
  });
}

LaneConfigResult buildLaneConfig({
  required VeilLane wsLane,
  VeilLane? quicLane,
  VeilLane? torLane,
  VeilLane? bleLane,
  bool ghostMode = false,
  bool p2pAnyLane = true,
}) {
  final ordered = <VeilLane>[
    if (ghostMode && torLane != null) torLane,
    if (ghostMode && bleLane != null) bleLane,
    if (quicLane != null) quicLane,
    wsLane,
    if (!ghostMode && torLane != null) torLane,
    if (!ghostMode && bleLane != null) bleLane,
  ];
  final p2pLanes = ordered.toList();
  final VeilLane fastLane;
  final VeilLane? fallbackLane;
  final VeilLane primaryLane = quicLane != null
      ? MultiLane(
          lanes: [quicLane, wsLane],
          sendMode: MultiLaneSendMode.primaryThenFallback,
        )
      : wsLane;
  if (p2pAnyLane && p2pLanes.length > 1) {
    fastLane = MultiLane(
      lanes: p2pLanes,
      sendMode: MultiLaneSendMode.broadcast,
    );
    fallbackLane = null;
  } else {
    if (ghostMode && torLane != null) {
      fastLane = torLane;
      fallbackLane = primaryLane;
    } else if (ghostMode && bleLane != null) {
      fastLane = bleLane;
      fallbackLane = primaryLane;
    } else {
      fastLane = primaryLane;
      fallbackLane = torLane ?? bleLane;
    }
  }
  return LaneConfigResult(
    fastLane: fastLane,
    fallbackLane: fallbackLane,
    wsLane: wsLane,
    quicLane: quicLane,
    p2pLanes: p2pLanes,
    publishLane: primaryLane,
  );
}
