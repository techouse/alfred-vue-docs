# encoding: utf-8


class Config(object):
    # Number of results to fetch from API
    RESULT_COUNT = 20
    # How long to cache results for
    CACHE_MAX_AGE = 20  # seconds
    # Icon
    VUE_ICON = "icon.png"
    GOOGLE_ICON = "google.png"
    # supported docs
    SUPPORTED_VUE_VERSIONS = {"v2", "v3"}
    DEFAULT_VUE_VERSION = "3"
    # Algolia credentials
    ALGOLIA_APP_ID = "ML0LEBN7FQ"
    ALGOLIA_SEARCH_ONLY_API_KEY = "f49cbd92a74532cc55cfbffa5e5a7d01"
    ALGOLIA_SEARCH_INDEX = "vuejs"
