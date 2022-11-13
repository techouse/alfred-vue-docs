import 'package:json_annotation/json_annotation.dart';

part 'search_result_hierarchy.g.dart';

@JsonSerializable()
class SearchResultHierarchy {
  const SearchResultHierarchy({
    required this.lvl0,
    this.lvl1,
    this.lvl2,
    this.lvl3,
    this.lvl4,
    this.lvl5,
    this.lvl6,
  });

  final String lvl0;
  final String? lvl1;
  final String? lvl2;
  final String? lvl3;
  final String? lvl4;
  final String? lvl5;
  final String? lvl6;

  String? getLevel(int level) {
    if (level == 1) return lvl1;
    if (level == 2) return lvl2;
    if (level == 3) return lvl3;
    if (level == 4) return lvl4;
    if (level == 5) return lvl5;
    if (level == 6) return lvl6;

    return lvl0;
  }

  String get first => lvl0;

  String get last => lvl6 ?? lvl5 ?? lvl4 ?? lvl3 ?? lvl2 ?? lvl1 ?? lvl0;

  factory SearchResultHierarchy.fromJson(Map<String, dynamic> json) =>
      _$SearchResultHierarchyFromJson(json);

  Map<String, String?> toJson() => _$SearchResultHierarchyToJson(this)
      .map((key, value) => MapEntry(key, value?.toString()));
}
