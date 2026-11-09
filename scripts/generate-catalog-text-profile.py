"""Freeze the legacy Unicode contract as data; not used at application runtime."""
import json
from pathlib import Path
import unicodedata


def ranges(predicate):
    result=[]
    for number in range(0x110000):
        if not predicate(chr(number)):continue
        if result and result[-1][1]+1==number:result[-1][1]=number
        else:result.append([number,number])
    return result


def main():
    folded={};normalized={};decomposed={};lowered={}
    for number in range(0x110000):
        char=chr(number)
        fold=char.casefold()
        if fold!=char:folded[char]=fold
        lower=char.lower()
        if lower!=char:lowered[char]=lower
        decomposition=unicodedata.normalize('NFKD',char)
        if decomposition!=char:decomposed[char]=decomposition
        value=''.join(c for c in unicodedata.normalize('NFKD',char).casefold() if c.isalnum())
        if value!=char and (value or char.isalnum()):normalized[char]=value
    profile={'schema_version':1,'unicode_version':unicodedata.unidata_version,
        'source':'Python unicodedata legacy catalog normalization contract',
        'casefold':folded,'normal':normalized,'nfkd':decomposed,'lower':lowered,'alnum':ranges(str.isalnum),
        'alpha':ranges(str.isalpha),'decimal':ranges(str.isdecimal),
        'combining':ranges(lambda c: bool(unicodedata.combining(c)))}
    path=Path(__file__).resolve().parents[1]/'native/catalog-core/data/text-profile-v1.json'
    path.write_text(json.dumps(profile,ensure_ascii=True,separators=(',',':'))+'\n',encoding='utf8')
    print(path);print('Unicode',profile['unicode_version'],'normalizations',len(normalized))


if __name__=='__main__':main()
