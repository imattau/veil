class ProfileData {
  final String pubkey;
  final String displayName;
  final String bio;
  final String? avatarMediaRoot;
  final String? lightningAddress;
  final int updatedAt;

  const ProfileData({
    required this.pubkey,
    required this.displayName,
    required this.bio,
    this.avatarMediaRoot,
    this.lightningAddress,
    required this.updatedAt,
  });

  factory ProfileData.fromEvent(dynamic event) {
    // This supports both NodeEvent and raw Map
    final Map<String, dynamic> data;
    final int seq;
    if (event is Map<String, dynamic>) {
      data = event['data'] as Map<String, dynamic>? ?? {};
      seq = event['seq'] as int? ?? 0;
    } else {
      // Assume NodeEvent
      data = event.data;
      seq = event.seq;
    }

    return ProfileData(
      pubkey: data['author_pubkey_hex'] as String? ?? 'unknown',
      displayName: data['display_name'] as String? ?? 'Anon',
      bio: data['bio'] as String? ?? '',
      avatarMediaRoot: data['avatar_media_root'] as String?,
      lightningAddress: data['lightning_address'] as String?,
      updatedAt: (data['meta']?['created_at'] as num?)?.toInt() ?? seq,
    );
  }

  ProfileData copyWith({
    String? pubkey,
    String? displayName,
    String? bio,
    String? avatarMediaRoot,
    String? lightningAddress,
    int? updatedAt,
  }) {
    return ProfileData(
      pubkey: pubkey ?? this.pubkey,
      displayName: displayName ?? this.displayName,
      bio: bio ?? this.bio,
      avatarMediaRoot: avatarMediaRoot ?? this.avatarMediaRoot,
      lightningAddress: lightningAddress ?? this.lightningAddress,
      updatedAt: updatedAt ?? this.updatedAt,
    );
  }
}
