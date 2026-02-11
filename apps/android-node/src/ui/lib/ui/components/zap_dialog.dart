import 'package:flutter/material.dart';
import '../../logic/zap_controller.dart';
import '../theme/veil_theme.dart';

class ZapDialog extends StatefulWidget {
  final String lnAddress;
  final String targetRoot;
  final String authorPubkey;
  final ZapController controller;

  const ZapDialog({
    super.key,
    required this.lnAddress,
    required this.targetRoot,
    required this.authorPubkey,
    required this.controller,
  });

  @override
  State<ZapDialog> createState() => _ZapDialogState();
}

class _ZapDialogState extends State<ZapDialog> {
  bool _isProcessing = false;
  int _selectedAmount = 100;

  Future<void> _handleZap() async {
    setState(() => _isProcessing = true);
    try {
      final invoice = await widget.controller.getInvoice(widget.lnAddress, _selectedAmount);
      if (invoice != null) {
        await widget.controller.launchWallet(invoice);
        // After launching, we broadcast social proof
        await widget.controller.broadcastZap(
          targetRoot: widget.targetRoot,
          amount: _selectedAmount,
          authorPubkey: widget.authorPubkey,
        );
        if (mounted) Navigator.pop(context);
      } else {
        throw 'Could not fetch invoice';
      }
    } catch (e) {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Zap failed: $e')),
        );
      }
    } finally {
      if (mounted) setState(() => _isProcessing = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Row(
        children: const [
          Icon(Icons.bolt, color: Colors.amber),
          SizedBox(width: 8),
          Text('Zap Post'),
        ],
      ),
      content: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            'Sending to ${widget.lnAddress}',
            style: const TextStyle(fontSize: 12, color: VeilTheme.textSecondary),
          ),
          const SizedBox(height: 24),
          Row(
            mainAxisAlignment: MainAxisAlignment.spaceEvenly,
            children: [50, 100, 1000].map((amt) {
              final isSelected = _selectedAmount == amt;
              return ChoiceChip(
                label: Text('$amt'),
                selected: isSelected,
                onSelected: (val) => setState(() => _selectedAmount = amt),
                selectedColor: Colors.amber.withOpacity(0.2),
                labelStyle: TextStyle(color: isSelected ? Colors.amber : Colors.white),
              );
            }).toList(),
          ),
        ],
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Cancel'),
        ),
        ElevatedButton(
          onPressed: _isProcessing ? null : _handleZap,
          style: ElevatedButton.styleFrom(backgroundColor: Colors.amber, foregroundColor: Colors.black),
          child: _isProcessing 
            ? const SizedBox(width: 20, height: 20, child: CircularProgressIndicator(strokeWidth: 2))
            : const Text('Zap!'),
        ),
      ],
    );
  }
}
