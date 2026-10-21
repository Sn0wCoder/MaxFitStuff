import json,re
def load(path):
    t=open(path,'rb').read().decode('utf-8','replace')
    t=re.sub(r'(:\s*)-?0+(\d)', lambda m:m.group(1)+('-' if '-' in m.group(0) else '')+m.group(2), t)
    t=re.sub(r':\s*0+(?=[,\s}])', ': 0', t)
    t=re.sub(r',(\s*[}\]])', r'\1', t)
    return json.loads(t)
