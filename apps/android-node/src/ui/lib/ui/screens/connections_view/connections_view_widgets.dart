part of '../connections_view.dart';

class _TabWithBadge extends StatelessWidget {
  final String label;
  final int count;

  const _TabWithBadge({required this.label, required this.count});

  @override
  Widget build(BuildContext context) {
    return Tab(
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(label),
          if (count > 0) ...[
            const SizedBox(width: 6),
            Container(
              padding: const EdgeInsets.symmetric(horizontal: 6, vertical: 1),
              decoration: BoxDecoration(
                color: VeilTheme.surfaceHighlight,
                borderRadius: BorderRadius.circular(8),
              ),
              child: Text(
                '$count',
                style: const TextStyle(
                  fontSize: 10,
                  fontWeight: FontWeight.w600,
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _UserTile extends StatelessWidget {
  final String pubkey;
  final String displayName;
  final String? bio;
  final Uint8List? avatarBytes;
  final String actionLabel;
  final Color actionColor;
  final VoidCallback onAction;
  final VoidCallback onTap;

  const _UserTile({
    required this.pubkey,
    required this.displayName,
    this.bio,
    this.avatarBytes,
    required this.actionLabel,
    required this.actionColor,
    required this.onAction,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final shortPk = pubkey.length > 16
        ? '${pubkey.substring(0, 8)}\u2026${pubkey.substring(pubkey.length - 8)}'
        : pubkey;
    final initial = displayName.isNotEmpty ? displayName[0].toUpperCase() : '?';
    final hasBio = bio != null && bio!.isNotEmpty;

    return GestureDetector(
      onTap: onTap,
      child: Container(
        margin: const EdgeInsets.only(bottom: 2),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 12),
        decoration: BoxDecoration(
          color: VeilTheme.surface,
          borderRadius: BorderRadius.circular(12),
          border: Border.all(color: Colors.white.withValues(alpha: 0.03)),
        ),
        child: Row(
          children: [
            CircleAvatar(
              radius: 20,
              backgroundColor: VeilTheme.accentSubtle,
              backgroundImage: avatarBytes != null
                  ? MemoryImage(avatarBytes!)
                  : null,
              child: avatarBytes == null
                  ? Text(
                      initial,
                      style: const TextStyle(
                        color: VeilTheme.accent,
                        fontSize: 16,
                        fontWeight: FontWeight.w700,
                      ),
                    )
                  : null,
            ),
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    displayName,
                    style: const TextStyle(
                      fontSize: 14,
                      fontWeight: FontWeight.w600,
                      color: VeilTheme.textPrimary,
                    ),
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                  ),
                  const SizedBox(height: 2),
                  Text(
                    shortPk,
                    style: TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 11,
                      color: VeilTheme.textSecondary.withValues(alpha: 0.5),
                      letterSpacing: 0.3,
                    ),
                  ),
                  if (hasBio) ...[
                    const SizedBox(height: 4),
                    Text(
                      bio!,
                      style: TextStyle(
                        fontSize: 12,
                        color: VeilTheme.textSecondary.withValues(alpha: 0.7),
                        height: 1.3,
                      ),
                      maxLines: 1,
                      overflow: TextOverflow.ellipsis,
                    ),
                  ],
                ],
              ),
            ),
            const SizedBox(width: 8),
            GestureDetector(
              onTap: () {
                HapticFeedback.lightImpact();
                onAction();
              },
              child: Container(
                padding: const EdgeInsets.symmetric(
                  horizontal: 10,
                  vertical: 5,
                ),
                decoration: BoxDecoration(
                  color: actionColor.withValues(alpha: 0.1),
                  borderRadius: BorderRadius.circular(6),
                  border: Border.all(color: actionColor.withValues(alpha: 0.2)),
                ),
                child: Text(
                  actionLabel,
                  style: TextStyle(
                    color: actionColor,
                    fontSize: 11,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _ActionSelector extends StatelessWidget {
  final String selected;
  final ValueChanged<String> onChanged;

  const _ActionSelector({required this.selected, required this.onChanged});

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        _ActionChip(
          label: 'Follow',
          value: 'follow',
          selected: selected,
          color: VeilTheme.accent,
          onTap: () => onChanged('follow'),
        ),
        const SizedBox(width: 8),
        _ActionChip(
          label: 'Mute',
          value: 'mute',
          selected: selected,
          color: const Color(0xFFD4A017),
          onTap: () => onChanged('mute'),
        ),
        const SizedBox(width: 8),
        _ActionChip(
          label: 'Block',
          value: 'block',
          selected: selected,
          color: const Color(0xFFE05252),
          onTap: () => onChanged('block'),
        ),
      ],
    );
  }
}

class _ActionChip extends StatelessWidget {
  final String label;
  final String value;
  final String selected;
  final Color color;
  final VoidCallback onTap;

  const _ActionChip({
    required this.label,
    required this.value,
    required this.selected,
    required this.color,
    required this.onTap,
  });

  @override
  Widget build(BuildContext context) {
    final isSelected = value == selected;
    return GestureDetector(
      onTap: onTap,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 7),
        decoration: BoxDecoration(
          color: isSelected
              ? color.withValues(alpha: 0.15)
              : Colors.transparent,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(
            color: isSelected
                ? color.withValues(alpha: 0.4)
                : Colors.white.withValues(alpha: 0.08),
          ),
        ),
        child: Text(
          label,
          style: TextStyle(
            color: isSelected ? color : VeilTheme.textSecondary,
            fontSize: 12,
            fontWeight: isSelected ? FontWeight.w600 : FontWeight.w400,
          ),
        ),
      ),
    );
  }
}

class _EmptyState extends StatelessWidget {
  final IconData icon;
  final String title;
  final String subtitle;

  const _EmptyState({
    required this.icon,
    required this.title,
    required this.subtitle,
  });

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 48),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              padding: const EdgeInsets.all(20),
              decoration: BoxDecoration(
                color: VeilTheme.surface,
                shape: BoxShape.circle,
                border: Border.all(color: Colors.white.withValues(alpha: 0.04)),
              ),
              child: Icon(
                icon,
                size: 32,
                color: VeilTheme.textSecondary.withValues(alpha: 0.3),
              ),
            ),
            const SizedBox(height: 20),
            Text(
              title,
              style: const TextStyle(
                fontSize: 16,
                fontWeight: FontWeight.w600,
                color: VeilTheme.textPrimary,
                letterSpacing: -0.3,
              ),
            ),
            const SizedBox(height: 6),
            Text(
              subtitle,
              textAlign: TextAlign.center,
              style: TextStyle(
                fontSize: 13,
                color: VeilTheme.textSecondary.withValues(alpha: 0.6),
                height: 1.4,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _StaggeredEntry extends StatefulWidget {
  final int index;
  final Widget child;

  const _StaggeredEntry({required this.index, required this.child});

  @override
  State<_StaggeredEntry> createState() => _StaggeredEntryState();
}

class _StaggeredEntryState extends State<_StaggeredEntry>
    with SingleTickerProviderStateMixin {
  late final AnimationController _animController;
  late final Animation<double> _fadeAnim;
  late final Animation<Offset> _slideAnim;

  @override
  void initState() {
    super.initState();
    final delay = (widget.index * 40).clamp(0, 300);
    _animController = AnimationController(
      duration: const Duration(milliseconds: 280),
      vsync: this,
    );
    _fadeAnim = CurvedAnimation(parent: _animController, curve: Curves.easeOut);
    _slideAnim = Tween<Offset>(
      begin: const Offset(0, 0.08),
      end: Offset.zero,
    ).animate(CurvedAnimation(parent: _animController, curve: Curves.easeOut));
    Future.delayed(Duration(milliseconds: delay), () {
      if (mounted) _animController.forward();
    });
  }

  @override
  void dispose() {
    _animController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return FadeTransition(
      opacity: _fadeAnim,
      child: SlideTransition(position: _slideAnim, child: widget.child),
    );
  }
}
