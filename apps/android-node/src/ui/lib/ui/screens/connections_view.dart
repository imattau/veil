import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../logic/node_service.dart';
import '../../logic/social_controller.dart';
import '../theme/veil_theme.dart';

part 'connections_view/connections_view_sections.dart';
part 'connections_view/connections_view_widgets.dart';

class ConnectionsView extends StatefulWidget {
  final NodeService service;
  final SocialController controller;

  const ConnectionsView({
    super.key,
    required this.service,
    required this.controller,
  });

  @override
  State<ConnectionsView> createState() => _ConnectionsViewState();
}

class _ConnectionsViewState extends State<ConnectionsView>
    with SingleTickerProviderStateMixin {
  late final TabController _tabController;
  final TextEditingController _searchController = TextEditingController();
  String _searchQuery = '';

  @override
  void initState() {
    super.initState();
    _tabController = TabController(length: 3, vsync: this);
    _searchController.addListener(() {
      setState(() => _searchQuery = _searchController.text.toLowerCase());
    });
  }

  @override
  void dispose() {
    _tabController.dispose();
    _searchController.dispose();
    super.dispose();
  }

  List<String> _filterPubkeys(Set<String> pubkeys) {
    final sorted = pubkeys.toList()..sort();
    if (_searchQuery.isEmpty) return sorted;
    return sorted.where((pk) {
      final profile = widget.service.profiles[pk];
      final name = (profile?.displayName ?? '').toLowerCase();
      return name.contains(_searchQuery) || pk.contains(_searchQuery);
    }).toList();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: VeilTheme.background,
      appBar: AppBar(
        backgroundColor: VeilTheme.background,
        elevation: 0,
        leading: IconButton(
          icon: const Icon(Icons.arrow_back_ios_new, size: 18),
          onPressed: () => Navigator.pop(context),
        ),
        title: const Text(
          'Connections',
          style: TextStyle(
            fontSize: 18,
            fontWeight: FontWeight.w700,
            letterSpacing: -0.3,
          ),
        ),
        bottom: PreferredSize(
          preferredSize: const Size.fromHeight(92),
          child: Column(children: [_buildTabBar(), _buildSearchBar()]),
        ),
      ),
      body: ListenableBuilder(
        listenable: widget.controller,
        builder: (context, _) {
          final following = _filterPubkeys(widget.controller.followedPubkeys);
          final muted = _filterPubkeys(widget.controller.mutedPubkeys);
          final blocked = _filterPubkeys(widget.controller.blockedPubkeys);

          return TabBarView(
            controller: _tabController,
            children: [
              _buildTab(
                pubkeys: following,
                emptyIcon: Icons.people_outline,
                emptyTitle: 'No one yet',
                emptySubtitle: 'Follow people to see their posts in your feed',
                actionLabel: 'Unfollow',
                actionColor: VeilTheme.textSecondary,
                onAction: (pk) => widget.controller.unfollowUser(pk),
                swipeColor: const Color(0xFF2A1A1A),
                swipeIcon: Icons.person_remove_outlined,
                swipeLabel: 'Unfollow',
              ),
              _buildTab(
                pubkeys: muted,
                emptyIcon: Icons.volume_off_outlined,
                emptyTitle: 'No muted users',
                emptySubtitle: 'Muted users won\'t appear in your feed',
                actionLabel: 'Unmute',
                actionColor: const Color(0xFFD4A017),
                onAction: (pk) => widget.controller.unmuteUser(pk),
                swipeColor: const Color(0xFF2A2410),
                swipeIcon: Icons.volume_up_outlined,
                swipeLabel: 'Unmute',
              ),
              _buildTab(
                pubkeys: blocked,
                emptyIcon: Icons.block_outlined,
                emptyTitle: 'No blocked users',
                emptySubtitle: 'Blocked users cannot interact with you',
                actionLabel: 'Unblock',
                actionColor: const Color(0xFFE05252),
                onAction: (pk) => widget.controller.unblockUser(pk),
                swipeColor: const Color(0xFF2A1414),
                swipeIcon: Icons.lock_open_outlined,
                swipeLabel: 'Unblock',
              ),
            ],
          );
        },
      ),
      floatingActionButton: _buildFAB(),
    );
  }
}
