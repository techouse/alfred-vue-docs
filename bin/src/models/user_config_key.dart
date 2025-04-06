import 'package:recase/recase.dart';

enum UserConfigKey {
  vueVersion,
  useAlfredCache,
  useFileCache,
  cacheTtl,
  fileCacheMaxEntries;

  @override
  String toString() => name.snakeCase;
}
