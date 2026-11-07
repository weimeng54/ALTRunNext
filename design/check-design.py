"""Offline checks using the existing Python environment; installs nothing."""
import json
import re
from pathlib import Path
import yaml

root = Path(__file__).resolve().parents[1]
html = (root / 'design/altrun-prototype.html').read_text(encoding='utf-8')
doc = (root / 'DESIGN.md').read_text(encoding='utf-8')
tokens = yaml.safe_load(doc.split('---', 2)[1])
mapping = {'background':'bg','surface':'surface','text':'text','muted':'muted','line':'line','blue':'blue','selected':'select','danger':'danger','ok':'ok'}
for name, css in mapping.items():
    expected = f"--ar-{css}:light-dark({tokens['colors'][name+'-light']},{tokens['colors'][name+'-dark']})"
    assert expected in html, expected
assert all(isinstance(v,dict) and v.get('fontFamily') for v in tokens['typography'].values())
assert all(re.fullmatch(r'#[0-9a-fA-F]{6}', v) for v in tokens['colors'].values())
ids = re.findall(r'\bid="([^"]+)"',html)
assert len(ids) == len(set(ids)), 'duplicate IDs'
assert not re.search(r'<!doctype|<html\b|<body\b|<head\b',html,re.I), 'not a fragment'
assert not re.search(r'\b(?:fetch|alert|confirm|prompt)\s*\(',html), 'unexpected browser API'
assert 'src="https://' not in html and 'href="https://' not in html

def luminance(hex):
    vals=[int(hex[i:i+2],16)/255 for i in (1,3,5)]
    vals=[v/12.92 if v<=0.04045 else ((v+.055)/1.055)**2.4 for v in vals]
    return sum(a*b for a,b in zip(vals,(.2126,.7152,.0722)))

contrast=[]
for theme in ['light','dark']:
    for text in ['text','muted','blue','danger','ok']:
        for bg in ['surface','background']:
            a,b=sorted((luminance(tokens['colors'][text+'-'+theme]),luminance(tokens['colors'][bg+'-'+theme])))
            ratio=(b+.05)/(a+.05)
            assert ratio>=4.5, (theme,text,bg,ratio)
            contrast.append({'theme':theme,'text':text,'surface':bg,'ratio':round(ratio,2)})
result={'status':'passed','tokenPairs':len(mapping),'uniqueIds':len(ids),'contrastPairs':contrast,'limits':'Offline checks are not official designmd lint or a full accessibility audit.'}
(root / 'design/qa/design-checks.json').write_text(json.dumps(result,ensure_ascii=False,indent=2),encoding='utf-8')
print(json.dumps({'status':'passed','tokenPairs':len(mapping),'contrastPairs':len(contrast),'uniqueIds':len(ids)}))
