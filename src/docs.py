#!/usr/bin/python
# encoding: utf-8

from __future__ import print_function, unicode_literals, absolute_import

import functools
import re
import sys
from collections import OrderedDict
from HTMLParser import HTMLParser
from textwrap import wrap
from urllib import quote_plus

from algoliasearch.search_client import SearchClient
from config import Config
from workflow import Workflow3, ICON_INFO

# log
log = None


def cache_key(query, version=Config.DEFAULT_VUE_VERSION):
    """Make filesystem-friendly cache key"""
    key = "{}_{}".format(query, version)
    key = key.lower()
    key = re.sub(r"[^a-z0-9-_;.]", "-", key)
    key = re.sub(r"-+", "-", key)
    # log.debug("Cache key : {!r} {!r} -> {!r}".format(query, version, key))
    return key


def handle_result(api_dict):
    """Extract relevant info from API result"""
    result = {}

    for key in {"objectID", "hierarchy", "content", "url", "type"}:
        if key == "hierarchy":
            for hierarchy_key, hierarchy_value in OrderedDict(
                sorted(api_dict[key].items(), reverse=True)
            ).items():
                if hierarchy_value:
                    result["title"] = hierarchy_value
                    break

            result["hierarchy"] = [
                value
                for value in OrderedDict(
                    sorted(api_dict[key].items(), key=lambda x: x[0])
                ).values()
                if value is not None
            ][:-1]

            result["subtitle"] = (
                " > ".join(result["hierarchy"]) if len(api_dict[key]) > 1 else None
            )
        else:
            result[key] = api_dict[key]

    return result


def search(query=None, version=Config.DEFAULT_VUE_VERSION, limit=Config.RESULT_COUNT):
    if query:
        # Algolia client
        client = SearchClient.create(
            Config.ALGOLIA_APP_ID,
            Config.ALGOLIA_SEARCH_ONLY_API_KEY,
        )
        # Algolia index
        index = client.init_index(Config.ALGOLIA_SEARCH_INDEX)
        # Get the results
        results = index.search(
            query,
            {
                "facetFilters": [
                    "version:v{}".format(version),
                ],
                "attributesToRetrieve": [
                    "hierarchy.lvl0",
                    "hierarchy.lvl1",
                    "hierarchy.lvl2",
                    "hierarchy.lvl3",
                    "hierarchy.lvl4",
                    "hierarchy.lvl5",
                    "hierarchy.lvl6",
                    "content",
                    "type",
                    "url",
                ],
                "attributesToSnippet": [
                    "hierarchy.lvl1:10",
                    "hierarchy.lvl2:10",
                    "hierarchy.lvl3:10",
                    "hierarchy.lvl4:10",
                    "hierarchy.lvl5:10",
                    "hierarchy.lvl6:10",
                    "content:10",
                ],
                "snippetEllipsisText": "...",
                "page": 0,
                "hitsPerPage": limit,
            },
        )
        if results is not None and "hits" in results:
            return results["hits"]
    return []


def main(wf):
    if wf.update_available:
        # Add a notification to top of Script Filter results
        wf.add_item(
            "New version available",
            "Action this item to install the update",
            autocomplete="workflow:update",
            icon=ICON_INFO,
        )

    query = wf.args[0].strip()

    # Tag prefix only. Treat as blank query
    if query == "v":
        query = ""

    if not query:
        wf.add_item("Search the Vue.js docs...")
        wf.send_feedback()
        return 0

    # Parse query into query string and tags
    words = query.split(" ")

    query = []
    version = Config.DEFAULT_VUE_VERSION

    for word in words:
        if word in Config.SUPPORTED_VUE_VERSIONS:
            version = word.replace("v", "")
        else:
            query.append(word)

    query = " ".join(query)

    # log.debug("version: {!r}".format(version))
    # log.debug("query: {!r}".format(query))

    key = cache_key(query, version)

    results = [
        handle_result(result)
        for result in wf.cached_data(
            key,
            functools.partial(search, query, version),
            max_age=Config.CACHE_MAX_AGE,
        )
    ]

    # log.debug("{} results for {!r}, version {!r}".format(len(results), query, version))

    sorted_results = OrderedDict()
    for result in results:
        hierarchy = result["hierarchy"]
        subtitle = result["subtitle"]

        if hierarchy[0] in sorted_results:
            if subtitle in sorted_results[hierarchy[0]]:
                sorted_results[hierarchy[0]][subtitle].append(result)
            else:
                sorted_results[hierarchy[0]][subtitle] = [result]
        else:
            sorted_results[hierarchy[0]] = OrderedDict()
            sorted_results[hierarchy[0]][subtitle] = [result]

    # log.debug(sorted_results)

    # Show results
    if not results:
        url = "https://www.google.com/search?q={}".format(
            quote_plus("Vue.js {}".format(query))
        )
        wf.add_item(
            "No matching answers found",
            "Shall I try and search Google?",
            valid=True,
            arg=url,
            copytext=url,
            quicklookurl=url,
            icon=Config.GOOGLE_ICON,
        )

    html_parser = HTMLParser()

    for group_name in sorted_results.keys():
        for key in sorted_results[group_name].keys():
            subtitle = wrap(key, width=75)[0]
            if len(subtitle) > 75:
                subtitle += "..."

            for result in sorted_results[group_name][key]:
                wf.add_item(
                    uid=result["objectID"],
                    title=html_parser.unescape(result["title"]),
                    subtitle=html_parser.unescape(subtitle),
                    arg=result["url"],
                    valid=True,
                    largetext=html_parser.unescape(result["title"]),
                    copytext=result["url"],
                    quicklookurl=result["url"],
                    icon=Config.VUE_ICON,
                )
                # log.debug(result)

    wf.send_feedback()


if __name__ == "__main__":
    wf = Workflow3(
        update_settings={
            "github_slug": "techouse/alfred-vue-docs",
            "frequency": 7,
        }
    )
    log = wf.logger
    sys.exit(wf.run(main))
