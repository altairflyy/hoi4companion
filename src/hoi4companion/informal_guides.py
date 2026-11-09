"""Conservative explanatory hints, never implicit game actions or conditions."""
from .research_upgrades import guide_language


LAWS = {name.casefold(): name for name in (
    'Civilian Economy', 'Early Mobilization', 'Partial Mobilization', 'War Economy', 'Total Mobilization',
    'Disarmed Nation', 'Volunteer Only', 'Limited Conscription', 'Extensive Conscription',
    'Service by Requirement', 'All Adults Serve', 'Scraping the Barrel',
    'Free Trade', 'Export Focus', 'Limited Exports', 'Closed Economy')}


def explanatory_terms(text, category=None):
    from .backend_core import _request
    return _request(0,'import_reading_terms',text=text,category=category,vocabulary=guide_language())


def explicit_action(text):
    from .backend_core import _request
    return _request(0,'import_explicit_action',text=text)


def abbreviation_hint(text):
    from .backend_core import _request
    return _request(0,'import_reading_hint',text=text,vocabulary=guide_language())
