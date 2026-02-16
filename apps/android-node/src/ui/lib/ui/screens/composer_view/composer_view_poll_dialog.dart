part of '../composer_view.dart';

class _PollDraft {
  final String question;
  final List<String> options;

  const _PollDraft({required this.question, required this.options});
}

class _CreatePollDialog extends StatefulWidget {
  const _CreatePollDialog();

  @override
  State<_CreatePollDialog> createState() => _CreatePollDialogState();
}

class _CreatePollDialogState extends State<_CreatePollDialog> {
  final TextEditingController _questionController = TextEditingController();
  final List<TextEditingController> _optionControllers = [
    TextEditingController(),
    TextEditingController(),
  ];
  String? _errorText;

  @override
  void dispose() {
    _questionController.dispose();
    for (final controller in _optionControllers) {
      controller.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Create Poll'),
      content: SingleChildScrollView(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            TextField(
              controller: _questionController,
              decoration: const InputDecoration(labelText: 'Question'),
            ),
            const SizedBox(height: 12),
            ..._optionControllers.asMap().entries.map((entry) {
              final index = entry.key;
              final controller = entry.value;
              return Padding(
                padding: const EdgeInsets.only(bottom: 8),
                child: TextField(
                  controller: controller,
                  decoration: InputDecoration(labelText: 'Option ${index + 1}'),
                ),
              );
            }),
            if (_optionControllers.length < 4)
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () {
                    setState(() {
                      _optionControllers.add(TextEditingController());
                    });
                  },
                  icon: const Icon(Icons.add),
                  label: const Text('Add option'),
                ),
              ),
            if (_errorText != null)
              Padding(
                padding: const EdgeInsets.only(top: 8),
                child: Text(
                  _errorText!,
                  style: const TextStyle(color: Colors.red, fontSize: 13),
                ),
              ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        TextButton(
          onPressed: () {
            final question = _questionController.text.trim();
            final options = _optionControllers
                .map((controller) => controller.text.trim())
                .where((text) => text.isNotEmpty)
                .toList();
            if (question.isEmpty || options.length < 2) {
              setState(() {
                _errorText = 'Provide a question and at least 2 options';
              });
              return;
            }
            Navigator.pop(
              context,
              _PollDraft(question: question, options: options),
            );
          },
          child: const Text('Create'),
        ),
      ],
    );
  }
}
