part of 'main.dart';

final HtmlUnescape _unescape = HtmlUnescape();

final AlfredWorkflow _workflow = AlfredWorkflow()
  ..disableAlfredSmartResultOrdering = true;

final AlfredUpdater _updater = AlfredUpdater(
  githubRepositoryUrl: Uri.parse(Env.githubRepositoryUrl),
  currentVersion: Env.appVersion,
  updateInterval: Duration(days: 7),
);

const _updateItem = AlfredItem(
  title: 'Auto-Update available!',
  subtitle: 'Press <enter> to auto-update to a new version of this workflow.',
  arg: 'update:workflow',
  match:
      'Auto-Update available! Press <enter> to auto-update to a new version of this workflow.',
  icon: AlfredItemIcon(path: 'alfredhatcog.png'),
  valid: true,
);

void _showPlaceholder() {
  _workflow.addItem(
    const AlfredItem(
      title: 'Search the Vue.js docs...',
      icon: AlfredItemIcon(path: 'icon.png'),
    ),
  );
}

Future<void> _performSearch(String query, {required String version}) async {
  try {
    final SearchResponse res = await AlgoliaSearch.query(
      query,
      version: version,
    );

    if (res.nbHits > 0) {
      final sortedResults = _sortResults(
        res.hits.map(
          (Hit hit) => SearchResult.fromJson(
            <String, dynamic>{...hit, 'objectID': hit.objectID},
          ),
        ),
      );

      final AlfredItems items = AlfredItems([]);

      for (final String groupName in sortedResults.keys) {
        for (final String key in sortedResults[groupName]!.keys) {
          final String subtitle = key.truncate(75);

          for (final SearchResult result in sortedResults[groupName]![key]!) {
            items.items.add(
              AlfredItem(
                uid: result.objectID,
                title: _unescape.convert(result.hierarchy.last),
                subtitle: subtitle,
                arg: result.url,
                text: AlfredItemText(
                  largeType: _unescape.convert(result.hierarchy.last),
                  copy: result.url,
                ),
                quickLookUrl: result.url,
                icon: AlfredItemIcon(path: 'icon.png'),
                valid: true,
              ),
            );
          }
        }
      }

      _workflow.addItems(items.items);
    } else {
      final Uri url =
          Uri.https('www.google.com', '/search', {'q': 'Vue.js $query'});

      _workflow.addItem(
        AlfredItem(
          title: 'No matching answers found',
          subtitle: 'Shall I try and search Google?',
          arg: url.toString(),
          text: AlfredItemText(copy: url.toString()),
          quickLookUrl: url.toString(),
          icon: AlfredItemIcon(path: 'google.png'),
          valid: true,
        ),
      );
    }
  } finally {
    AlgoliaSearch.dispose();
  }
}

Map<String, Map<String, List<SearchResult>>> _sortResults(
  Iterable<SearchResult> results,
) {
  final Map<String, Map<String, List<SearchResult>>> sortedResults = {};

  for (final SearchResult result in results) {
    final Map<String, String?> hierarchy = result.hierarchy.toJson()
      ..removeWhere(
        (_, value) => value == null || value == result.hierarchy.last,
      );
    final String subtitle = hierarchy.isNotEmpty
        ? _unescape.convert(hierarchy.values.join(' > '))
        : '';
    final String groupName = result.hierarchy.first;

    if (sortedResults.containsKey(groupName)) {
      if (sortedResults[groupName]!.containsKey(subtitle)) {
        sortedResults[groupName]![subtitle]!.add(result);
      } else {
        sortedResults[groupName]![subtitle] = [result];
      }
    } else {
      sortedResults[groupName] = {
        subtitle: [result],
      };
    }
  }

  return sortedResults;
}
