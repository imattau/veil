import 'package:flutter/material.dart';

import '../app_controller.dart';
import 'widgets.dart';
class OnboardingScreen extends StatefulWidget {
  final VeilAppController controller;
  final VoidCallback onComplete;

  const OnboardingScreen({
    super.key,
    required this.controller,
    required this.onComplete,
  });

  @override
  State<OnboardingScreen> createState() => _OnboardingScreenState();
}

class _OnboardingScreenState extends State<OnboardingScreen> {
  final _nameController = TextEditingController();
  final _formKey = GlobalKey<FormState>();
  String _selected = 'Public Square';

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    return Scaffold(
      body: Container(
        decoration: const BoxDecoration(
          gradient: LinearGradient(
            colors: [Color(0xFF0B0E14), Color(0xFF111827)],
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
          ),
        ),
        child: SafeArea(
          child: LayoutBuilder(
            builder: (context, constraints) {
              return SingleChildScrollView(
                padding: const EdgeInsets.all(24),
                child: ConstrainedBox(
                  constraints: BoxConstraints(minHeight: constraints.maxHeight),
                  child: IntrinsicHeight(
                    child: Form(
                      key: _formKey,
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const SizedBox(height: 12),
                          Image.asset(
                            'assets/veil_header.png',
                            height: 120,
                            fit: BoxFit.cover,
                          ),
                          const SizedBox(height: 20),
                          Text(
                            'Welcome to VEIL',
                            style: Theme.of(context).textTheme.headlineMedium,
                          ),
                          const SizedBox(height: 8),
                          Text(
                            'Your identity is created automatically. Choose a display name and a starting space.',
                            style: Theme.of(
                              context,
                            ).textTheme.bodyMedium?.copyWith(color: Colors.white70),
                          ),
                          const SizedBox(height: 20),
                          InputField(
                            label: 'Display name',
                            controller: _nameController,
                            errorText: null, // Validation handled by Form logic below if we migrated InputField to TextFormField.
                            // InputField in widgets.dart is a wrapper around TextField, not TextFormField.
                            // I should verify InputField implementation or swap it.
                          ),
                          // To avoid changing InputField signature globally right now, I'll do manual validation check in onPressed.
                          
                          const SizedBox(height: 8),
                          DropdownButtonFormField<String>(
                            value: _selected,
                            decoration: const InputDecoration(
                              labelText: 'Start in',
                              filled: true,
                            ),
                            items: const [
                              DropdownMenuItem(
                                value: 'Public Square',
                                child: Text('Public Square (Open)'),
                              ),
                              DropdownMenuItem(
                                value: 'Private Circles',
                                child: Text('Private Circles (Encrypted)'),
                              ),
                            ],
                            onChanged: (value) {
                              if (value != null) {
                                setState(() => _selected = value);
                              }
                            },
                          ),
                          const SizedBox(height: 12),
                          Text(
                            'You can trust starter feeds later in Discovery.',
                            style: Theme.of(
                              context,
                            ).textTheme.bodySmall?.copyWith(
                              color: Colors.white70,
                            ),
                          ),
                          const Spacer(),
                          ElevatedButton(
                            onPressed: () {
                              if (_nameController.text.trim().isEmpty) {
                                ScaffoldMessenger.of(context).showSnackBar(
                                  const SnackBar(
                                    content: Text('Please enter a display name.'),
                                    backgroundColor: Colors.redAccent,
                                  ),
                                );
                                return;
                              }
                              controller.setDisplayName(_nameController.text);
                              controller.setNamespaceChoice(_selected);
                              controller.generateIdentity();
                              widget.onComplete();
                            },
                            style: ElevatedButton.styleFrom(
                              minimumSize: const Size.fromHeight(52),
                            ),
                            child: const Text('Continue'),
                          ),
                          const SizedBox(height: 12),
                          Text(
                            'Recovery phrase stored locally. You can export it later.',
                            style: Theme.of(
                              context,
                            ).textTheme.bodySmall?.copyWith(color: Colors.white60),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
              );
            },
          ),
        ),
      ),
    );
  }
}
