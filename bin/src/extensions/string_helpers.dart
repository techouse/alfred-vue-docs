import 'dart:convert' show utf8;

import 'package:crypto/crypto.dart' as crypto show md5;

extension StringHelpers on String {
  String get md5hex => crypto.md5.convert(utf8.encode(this)).toString();

  /// Truncate a string if it's longer than [maxLength] and add an [ellipsis].
  String truncate(int maxLength, [String ellipsis = '...']) =>
      length > maxLength
          ? '${substring(0, maxLength - ellipsis.length)}$ellipsis'
          : this;
}
