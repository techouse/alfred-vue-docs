import 'dart:io' show exitCode, stdout;

import 'package:alfred_workflow/alfred_workflow.dart'
    show AlfredItem, AlfredItemIcon, AlfredItemText, AlfredWorkflow;
import 'package:algolia/algolia.dart' show AlgoliaQuerySnapshot;
import 'package:args/args.dart' show ArgParser, ArgResults;
import 'package:collection/collection.dart' show IterableExtension;
import 'package:html_unescape/html_unescape.dart' show HtmlUnescape;

import 'src/constants/config.dart' show Config;
import 'src/extensions/truncate.dart' show Truncate;
import 'src/models/search_result.dart' show SearchResult;
import 'src/services/algolia_search.dart' show AlgoliaSearch;

void _showPlaceholder() {
  workflow.addItem(
    const AlfredItem(
      title: 'Search the Vue.js docs...',
      icon: AlfredItemIcon(path: 'icon.png'),
    ),
  );
}

Future<void> _performSearch(String query, {String? version}) async {
  final AlgoliaQuerySnapshot snapshot = await AlgoliaSearch.query(
    query,
    version: version,
  );

  if (snapshot.nbHits > 0) {
    final sortedResults = _sortResults(
      snapshot.hits.map(
        (snapshot) => SearchResult.fromJson(snapshot.data),
      ),
    );

    for (final String groupName in sortedResults.keys) {
      for (final String key in sortedResults[groupName]!.keys) {
        final String subtitle = key.truncate(75);

        for (final SearchResult result in sortedResults[groupName]![key]!) {
          workflow.addItem(
            AlfredItem(
              uid: result.objectID,
              title: unescape.convert(result.hierarchy.last),
              subtitle: subtitle,
              arg: result.url,
              text: AlfredItemText(
                largeType: unescape.convert(result.hierarchy.last),
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
  } else {
    final Uri url =
        Uri.https('www.google.com', '/search', {'q': 'Vue.js $query'});

    workflow.addItem(
      AlfredItem(
        title: 'No matching answers found',
        subtitle: 'Shall I try and search Google?',
        arg: url.toString(),
        text: AlfredItemText(
          copy: url.toString(),
        ),
        quickLookUrl: url.toString(),
        icon: AlfredItemIcon(path: 'google.png'),
        valid: true,
      ),
    );
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
    final String subtitle = hierarchy.length > 0
        ? unescape.convert(hierarchy.values.join(' > '))
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

final AlfredWorkflow workflow = AlfredWorkflow();
final HtmlUnescape unescape = HtmlUnescape();
bool verbose = false;

void main(List<String> arguments) async {
  try {
    exitCode = 0;

    workflow.clearItems();

    final ArgParser parser = ArgParser()
      ..addOption('query', abbr: 'q', mandatory: true)
      ..addFlag('verbose', abbr: 'v', defaultsTo: false);
    final ArgResults args = parser.parse(arguments);

    List<String> query =
        args['query'].replaceAll(RegExp(r'\s+'), ' ').trim().split(' ');
    String? version = query.firstWhereOrNull(
      (el) => Config.supportedVersions.contains(el),
    );
    if (version != null) {
      query.removeWhere((str) => str == version);
    } else {
      version = Config.supportedVersions.last;
    }
    final String queryString = query.join(' ').trim();

    if (args['verbose']) verbose = true;

    if (verbose) stdout.writeln('Query: "$queryString"');

    if (queryString.isEmpty) {
      _showPlaceholder();
    } else {
      await _performSearch(
        queryString,
        version: version,
      );
    }
  } on FormatException catch (err) {
    exitCode = 2;
    workflow.addItem(AlfredItem(title: err.toString()));
  } catch (err) {
    exitCode = 1;
    workflow.addItem(AlfredItem(title: err.toString()));
    if (verbose) {
      rethrow;
    }
  } finally {
    workflow.run();
  }
}
