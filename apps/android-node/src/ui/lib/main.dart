import 'package:flutter/material.dart';
import 'ui/screens/node_home.dart';

void main() {
  runApp(const VeilNodeApp());
}

class VeilNodeApp extends StatelessWidget {
  const VeilNodeApp({super.key});

  @override
  Widget build(BuildContext context) {
    final colorScheme = ColorScheme.fromSeed(
      seedColor: const Color(0xFF0B1D26),
      brightness: Brightness.light,
    );
    return MaterialApp(
      title: 'Veil Node',
      theme: ThemeData(
        colorScheme: colorScheme,
        useMaterial3: true,
        scaffoldBackgroundColor: const Color(0xFFF5F4F1),
      ),
      home: const NodeHome(),
    );
  }
}
