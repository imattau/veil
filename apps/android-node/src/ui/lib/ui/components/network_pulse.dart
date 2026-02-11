import 'package:flutter/material.dart';
import '../../logic/node_service.dart';
import '../theme/veil_theme.dart';

class NetworkPulse extends StatefulWidget {
  final NodeService service;

  const NetworkPulse({super.key, required this.service});

  @override
  State<NetworkPulse> createState() => _NetworkPulseState();
}

class _NetworkPulseState extends State<NetworkPulse> with SingleTickerProviderStateMixin {
  late AnimationController _pulseController;

  @override
  void initState() {
    super.initState();
    _pulseController = AnimationController(
      vsync: this,
      duration: const Duration(milliseconds: 800),
    );
  }

  @override
  void dispose() {
    _pulseController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: widget.service,
      builder: (context, _) {
        final status = widget.service.state.statusPayload;
        final quic = status['lanes']?['quic']?['connected'] == true;
        final ws = status['lanes']?['websocket']?['connected'] == true;
        final pending = (status['queue']?['pending'] as num?)?.toInt() ?? 0;
        
        Color color = Colors.red;
        String label = 'Disconnected';
        
        if (quic) {
          color = VeilTheme.accent;
          label = 'Fast';
        } else if (ws) {
          color = Colors.amber;
          label = 'Stable';
        }

        if (pending > 0 && !_pulseController.isAnimating) {
          _pulseController.repeat(reverse: true);
        } else if (pending == 0 && _pulseController.isAnimating) {
          _pulseController.stop();
        }

        return Tooltip(
          message: 'Network: $label ${pending > 0 ? '($pending sending)' : ''}',
          child: AnimatedBuilder(
            animation: _pulseController,
            builder: (context, child) {
              return Container(
                width: 10,
                height: 10,
                decoration: BoxDecoration(
                  color: color,
                  shape: BoxShape.circle,
                  boxShadow: [
                    BoxShadow(
                      color: color.withOpacity(0.5 + (0.5 * _pulseController.value)),
                      blurRadius: 4 + (4 * _pulseController.value),
                      spreadRadius: 2 + (2 * _pulseController.value),
                    ),
                  ],
                ),
              );
            },
          ),
        );
      },
    );
  }
}