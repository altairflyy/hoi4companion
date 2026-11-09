"""Decode the already acquired Paradox clock, without reading game memory.

Jomini 0.37.2 common/date.rs ExpandedRawDate::from_binary: zero-based
hours, 365-day years, year offset 5000. The calendar has no leap days.
"""
import re

MONTH_DAYS=(31,28,31,30,31,30,31,31,30,31,30,31)


def calendar_date(sample):
    if not isinstance(sample,dict):return None
    tick=sample.get('native_tick')
    if (sample.get('country_identity_validation')!='primary_selector_and_country_map'
            or not isinstance(sample.get('session_id'),str) or not sample['session_id']
            or not isinstance(sample.get('executable_sha256'),str)
            or not re.fullmatch('[0-9a-f]{64}',sample['executable_sha256'])
            or type(tick) is not int or not 0<=tick<=2147483647):return None
    days=tick//24;year=days//365-5000;day=days%365
    if not 1<=year<=9999:return None
    for month,length in enumerate(MONTH_DAYS,1):
        if day<length:return year*10000+month*100+day+1
        day-=length
