import 'package:flutter/material.dart';
import './ui/theme/veil_theme.dart';
import './ui/screens/social_home.dart';

void main() {
  runApp(const VeilApp());
}

class VeilApp extends StatelessWidget {
  const VeilApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'VEIL Social',
      debugShowCheckedModeBanner: false,
      theme: VeilTheme.dark,
      home: const SocialHome(),
    );
  }
}
