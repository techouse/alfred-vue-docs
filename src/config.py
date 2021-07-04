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
    ALGOLIA_CREDENTIALS = {
        "2": {
            "ALGOLIA_APP_ID": "BH4D9OD16A",
            "ALGOLIA_SEARCH_ONLY_API_KEY": "85cc3221c9f23bfbaa4e3913dd7625ea",
            "ALGOLIA_SEARCH_INDEX": "vuejs",
        },
        "3": {
            "ALGOLIA_APP_ID": "BH4D9OD16A",
            "ALGOLIA_SEARCH_ONLY_API_KEY": "bc6e8acb44ed4179c30d0a45d6140d3f",
            "ALGOLIA_SEARCH_INDEX": "vuejs-v3",
        }
    }
