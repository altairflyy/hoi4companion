"""Application language is independent of game localization and authored guides."""
from functools import lru_cache
from html import escape
from html.parser import HTMLParser
import json
from pathlib import Path
import re

DEFAULT_LANGUAGE='english'
LANGUAGES=('english','italian')
TEXT_FIELDS={'error','message','reason','hint','warning','warnings','status_message','help'}
CONTENT_FIELDS={'guide','tracks','sources','source','evidence','source_inventory','instructions','assumptions','params','note'}


@lru_cache(maxsize=1)
def catalog():
    return json.loads(Path(__file__).with_name('translations.json').read_text(encoding='utf-8'))


def language(value):
    return 'italian' if value in ('italian','it') else DEFAULT_LANGUAGE


@lru_cache(maxsize=2)
def patterns(locale):
    pairs=catalog()['legacy'] if locale=='english' else catalog()['messages']
    result=[]
    for original,target in pairs.items():
        parts=re.split(r'(\{\d+\})',original)
        if len(parts)==1:continue
        groups=[];chunks=[]
        for part in parts:
            if re.fullmatch(r'\{\d+\}',part):
                groups.append(part);chunks.append('(.*?)')
            else:chunks.append(re.escape(part))
        result.append((original.split('{',1)[0],re.compile('^'+''.join(chunks)+'$',re.S),groups,target))
    return result


def translate(message,locale=DEFAULT_LANGUAGE):
    if not isinstance(message,str):return message
    locale=language(locale)
    pairs=catalog()['legacy'] if locale=='english' else catalog()['messages']
    if message in pairs:return pairs[message]
    for prefix,pattern,groups,target in patterns(locale):
        if prefix and not message.startswith(prefix):continue
        match=pattern.fullmatch(message)
        if match:
            values=dict(zip(groups,match.groups()))
            return re.sub(r'\{\d+\}',lambda m:values.get(m.group(),m.group()),target)
    return message


def localize_payload(value,locale=DEFAULT_LANGUAGE):
    """Translate application diagnostics, never IDs, guide text or game names."""
    def visit(item,field=None):
        if field in CONTENT_FIELDS:return item
        if isinstance(item,dict) and 'tracks' in item and 'schema_version' in item:return item
        if isinstance(item,dict):return {key:visit(child,key) for key,child in item.items()}
        if isinstance(item,list):return [visit(child,field) for child in item]
        return translate(item,locale) if isinstance(item,str) and field in TEXT_FIELDS else item
    return visit(value)


class _Html(HTMLParser):
    def __init__(self,locale):
        super().__init__(convert_charrefs=True);self.locale=language(locale);self.output=[];self.raw=0
    def handle_starttag(self,tag,attrs):
        source=self.get_starttag_text()
        for key,value in attrs:
            if value is None:continue
            replacement=('it' if self.locale=='italian' else 'en') if tag=='html' and key=='lang' else translate(value,self.locale) if key in ('title','alt','placeholder','aria-label') else value
            if replacement!=value:
                source=re.sub(r'(\b'+re.escape(key)+r'\s*=\s*)([\"\'])(.*?)\2',
                    lambda m:m[1]+m[2]+escape(replacement,quote=True)+m[2],source,count=1,flags=re.S)
        self.output.append(source)
        if tag in ('script','style'):self.raw+=1
    def handle_endtag(self,tag):
        self.output.append('</'+tag+'>')
        if tag in ('script','style'):self.raw=max(0,self.raw-1)
    def handle_startendtag(self,tag,attrs):
        self.handle_starttag(tag,attrs)
    def handle_data(self,data):
        if self.raw:self.output.append(data);return
        clean=data.strip();text=translate(clean,self.locale)
        self.output.append(escape(data,quote=False) if text==clean else data[:len(data)-len(data.lstrip())]+escape(text,quote=False)+data[len(data.rstrip()):])
    def handle_entityref(self,name):self.output.append('&'+name+';')
    def handle_charref(self,name):self.output.append('&#'+name+';')
    def handle_decl(self,decl):self.output.append('<!'+decl+'>')
    def handle_comment(self,data):self.output.append('<!--'+data+'-->')


def localize_html(text,locale=DEFAULT_LANGUAGE):
    parser=_Html(locale);parser.feed(text);parser.close()
    return ''.join(parser.output)
