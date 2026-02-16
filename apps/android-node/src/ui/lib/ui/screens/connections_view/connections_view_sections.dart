part of '../connections_view.dart';

extension _ConnectionsViewSections on _ConnectionsViewState {
  Widget _buildTabBar() {
    return ListenableBuilder(
      listenable: widget.controller,
      builder: (context, _) {
        final followCount = widget.controller.followedPubkeys.length;
        final mutedCount = widget.controller.mutedPubkeys.length;
        final blockedCount = widget.controller.blockedPubkeys.length;

        return TabBar(
          controller: _tabController,
          indicatorColor: VeilTheme.accent,
          indicatorWeight: 2,
          indicatorSize: TabBarIndicatorSize.label,
          labelColor: VeilTheme.accent,
          unselectedLabelColor: VeilTheme.textSecondary,
          labelStyle: const TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.w600,
            letterSpacing: 0.3,
          ),
          unselectedLabelStyle: const TextStyle(
            fontSize: 13,
            fontWeight: FontWeight.w500,
          ),
          dividerColor: Colors.transparent,
          tabs: [
            _TabWithBadge(label: 'Following', count: followCount),
            _TabWithBadge(label: 'Muted', count: mutedCount),
            _TabWithBadge(label: 'Blocked', count: blockedCount),
          ],
        );
      },
    );
  }

  Widget _buildSearchBar() {
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 8, 16, 8),
      child: Container(
        height: 36,
        decoration: BoxDecoration(
          color: VeilTheme.surface,
          borderRadius: BorderRadius.circular(10),
          border: Border.all(color: Colors.white.withValues(alpha: 0.04)),
        ),
        child: TextField(
          controller: _searchController,
          style: const TextStyle(fontSize: 13, color: VeilTheme.textPrimary),
          decoration: InputDecoration(
            hintText: 'Filter by name or pubkey...',
            hintStyle: TextStyle(
              fontSize: 13,
              color: VeilTheme.textSecondary.withValues(alpha: 0.5),
            ),
            prefixIcon: Icon(
              Icons.search,
              size: 16,
              color: VeilTheme.textSecondary.withValues(alpha: 0.5),
            ),
            suffixIcon: _searchQuery.isNotEmpty
                ? GestureDetector(
                    onTap: () => _searchController.clear(),
                    child: Icon(
                      Icons.close,
                      size: 14,
                      color: VeilTheme.textSecondary.withValues(alpha: 0.5),
                    ),
                  )
                : null,
            border: InputBorder.none,
            contentPadding: const EdgeInsets.symmetric(
              horizontal: 12,
              vertical: 8,
            ),
            isDense: true,
          ),
        ),
      ),
    );
  }

  Widget _buildTab({
    required List<String> pubkeys,
    required IconData emptyIcon,
    required String emptyTitle,
    required String emptySubtitle,
    required String actionLabel,
    required Color actionColor,
    required Future<void> Function(String) onAction,
    required Color swipeColor,
    required IconData swipeIcon,
    required String swipeLabel,
  }) {
    if (pubkeys.isEmpty) {
      return _EmptyState(
        icon: emptyIcon,
        title: emptyTitle,
        subtitle: emptySubtitle,
      );
    }

    return ListView.builder(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      itemCount: pubkeys.length,
      itemBuilder: (context, index) {
        final pk = pubkeys[index];
        final profile = widget.service.profiles[pk];
        final avatarRoot = profile?.avatarMediaRoot;
        final avatarBytes = avatarRoot != null
            ? widget.controller.imageCache[avatarRoot]
            : null;

        return _StaggeredEntry(
          index: index,
          child: Dismissible(
            key: ValueKey('$actionLabel-$pk'),
            direction: DismissDirection.endToStart,
            background: Container(
              alignment: Alignment.centerRight,
              padding: const EdgeInsets.only(right: 24),
              margin: const EdgeInsets.only(bottom: 2),
              decoration: BoxDecoration(
                color: swipeColor,
                borderRadius: BorderRadius.circular(12),
              ),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(
                    swipeLabel,
                    style: TextStyle(
                      color: actionColor,
                      fontSize: 12,
                      fontWeight: FontWeight.w600,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Icon(swipeIcon, color: actionColor, size: 18),
                ],
              ),
            ),
            confirmDismiss: (direction) async {
              HapticFeedback.mediumImpact();
              await onAction(pk);
              return true;
            },
            child: _UserTile(
              pubkey: pk,
              displayName: widget.controller.getDisplayName(pk),
              bio: profile?.bio,
              avatarBytes: avatarBytes,
              actionLabel: actionLabel,
              actionColor: actionColor,
              onAction: () => onAction(pk),
              onTap: () {
                Clipboard.setData(ClipboardData(text: pk));
                HapticFeedback.lightImpact();
                ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(
                    content: Text('Public key copied'),
                    duration: Duration(seconds: 1),
                  ),
                );
              },
            ),
          ),
        );
      },
    );
  }

  Widget _buildFAB() {
    return FloatingActionButton(
      backgroundColor: VeilTheme.accent,
      foregroundColor: VeilTheme.background,
      elevation: 0,
      shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(16)),
      onPressed: () => _showAddConnectionDialog(),
      child: const Icon(Icons.person_add_outlined, size: 22),
    );
  }

  Future<void> _showAddConnectionDialog() async {
    final pubkeyController = TextEditingController();
    String selectedAction = 'follow';

    final result = await showDialog<Map<String, String>>(
      context: context,
      builder: (ctx) => StatefulBuilder(
        builder: (ctx, setDialogState) {
          return AlertDialog(
            backgroundColor: VeilTheme.surface,
            shape: RoundedRectangleBorder(
              borderRadius: BorderRadius.circular(16),
              side: BorderSide(color: Colors.white.withValues(alpha: 0.06)),
            ),
            title: const Text(
              'Add Connection',
              style: TextStyle(
                fontSize: 17,
                fontWeight: FontWeight.w700,
                letterSpacing: -0.3,
              ),
            ),
            content: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'PUBLIC KEY',
                  style: TextStyle(
                    fontSize: 10,
                    fontWeight: FontWeight.w600,
                    letterSpacing: 1.2,
                    color: VeilTheme.textSecondary.withValues(alpha: 0.6),
                  ),
                ),
                const SizedBox(height: 6),
                TextField(
                  controller: pubkeyController,
                  style: const TextStyle(
                    fontFamily: 'monospace',
                    fontSize: 12,
                    color: VeilTheme.textPrimary,
                  ),
                  maxLines: 2,
                  decoration: InputDecoration(
                    hintText: '64-character hex public key',
                    hintStyle: TextStyle(
                      fontFamily: 'monospace',
                      fontSize: 12,
                      color: VeilTheme.textSecondary.withValues(alpha: 0.4),
                    ),
                    filled: true,
                    fillColor: VeilTheme.background,
                    border: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(10),
                      borderSide: BorderSide(
                        color: Colors.white.withValues(alpha: 0.06),
                      ),
                    ),
                    enabledBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(10),
                      borderSide: BorderSide(
                        color: Colors.white.withValues(alpha: 0.06),
                      ),
                    ),
                    focusedBorder: OutlineInputBorder(
                      borderRadius: BorderRadius.circular(10),
                      borderSide: const BorderSide(
                        color: VeilTheme.accent,
                        width: 1,
                      ),
                    ),
                    contentPadding: const EdgeInsets.all(12),
                    isDense: true,
                    suffixIcon: IconButton(
                      icon: Icon(
                        Icons.content_paste,
                        size: 16,
                        color: VeilTheme.textSecondary.withValues(alpha: 0.5),
                      ),
                      onPressed: () async {
                        final data = await Clipboard.getData('text/plain');
                        if (data?.text != null) {
                          pubkeyController.text = data!.text!.trim();
                        }
                      },
                    ),
                  ),
                ),
                const SizedBox(height: 16),
                Text(
                  'ACTION',
                  style: TextStyle(
                    fontSize: 10,
                    fontWeight: FontWeight.w600,
                    letterSpacing: 1.2,
                    color: VeilTheme.textSecondary.withValues(alpha: 0.6),
                  ),
                ),
                const SizedBox(height: 6),
                _ActionSelector(
                  selected: selectedAction,
                  onChanged: (v) => setDialogState(() => selectedAction = v),
                ),
              ],
            ),
            actions: [
              TextButton(
                onPressed: () => Navigator.pop(ctx),
                child: Text(
                  'Cancel',
                  style: TextStyle(
                    color: VeilTheme.textSecondary.withValues(alpha: 0.7),
                    fontSize: 13,
                  ),
                ),
              ),
              TextButton(
                onPressed: () => Navigator.pop(ctx, {
                  'pubkey': pubkeyController.text.trim().toLowerCase(),
                  'action': selectedAction,
                }),
                style: TextButton.styleFrom(
                  backgroundColor: VeilTheme.accentSubtle,
                  shape: RoundedRectangleBorder(
                    borderRadius: BorderRadius.circular(8),
                  ),
                  padding: const EdgeInsets.symmetric(
                    horizontal: 16,
                    vertical: 8,
                  ),
                ),
                child: const Text(
                  'Apply',
                  style: TextStyle(
                    color: VeilTheme.accent,
                    fontSize: 13,
                    fontWeight: FontWeight.w600,
                  ),
                ),
              ),
            ],
          );
        },
      ),
    );

    if (result == null) return;
    final pubkey = result['pubkey'] ?? '';
    final action = result['action'] ?? 'follow';

    if (!RegExp(r'^[0-9a-f]{64}$').hasMatch(pubkey)) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        const SnackBar(content: Text('Enter a valid 64-character hex pubkey')),
      );
      return;
    }

    switch (action) {
      case 'follow':
        await widget.controller.followUser(pubkey);
        break;
      case 'mute':
        await widget.controller.muteUser(pubkey);
        break;
      case 'block':
        await widget.controller.blockUser(pubkey);
        break;
    }

    if (!mounted) return;
    final err = widget.service.state.lastError;
    ScaffoldMessenger.of(context).showSnackBar(
      SnackBar(
        content: Text(
          err ??
              switch (action) {
                'follow' => 'Followed',
                'mute' => 'Muted',
                'block' => 'Blocked',
                _ => 'Done',
              },
        ),
        duration: const Duration(seconds: 1),
      ),
    );
  }
}
