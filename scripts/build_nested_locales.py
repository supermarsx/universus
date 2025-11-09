import json, os

base = os.getcwd()
map_path = os.path.join(base, 'scripts', 'proposed-translation-mapping.json')
keys_path = os.path.join(base, 'scripts', 'extract-translation-keys.txt')
en_path = os.path.join(base, 'frontend', 'locales', 'en-US.json')
pt_path = os.path.join(base, 'frontend', 'locales', 'pt-PT.json')

def load_json(p):
    if os.path.exists(p):
        with open(p, 'r', encoding='utf-8') as f:
            return json.load(f)
    return {}

mapping = load_json(map_path)
en_flat = load_json(en_path)
pt_flat = load_json(pt_path)

# Flatten en_flat keys (some already are dotted like 'ui.open_notifications')
# Build en_lookup and pt_lookup mapping simple key -> value

en_lookup = {}
for k,v in en_flat.items():
    # If key contains a dot, try to use last segment as fallback key as well
    en_lookup[k] = v
    if '.' in k:
        en_lookup[k.split('.')[-1]] = en_lookup.get(k.split('.')[-1], v)

pt_lookup = {}
for k,v in pt_flat.items():
    pt_lookup[k] = v
    if '.' in k:
        pt_lookup[k.split('.')[-1]] = pt_lookup.get(k.split('.')[-1], v)

# Build nested dicts

nested_en = {}
nested_pt = {}

def set_nested(d, path, value):
    parts = path.split('.')
    cur = d
    for p in parts[:-1]:
        if p not in cur or not isinstance(cur[p], dict):
            cur[p] = {}
        cur = cur[p]
    cur[parts[-1]] = value

with open(keys_path, 'r', encoding='utf-8') as f:
    keys = [l.strip() for l in f if l.strip()]

for key in keys:
    target = mapping.get(key, key)
    # Try pt first, then en
    value = pt_lookup.get(key) or pt_lookup.get(target) or en_lookup.get(key) or en_lookup.get(target) or ''
    # For en also
    en_value = en_lookup.get(key) or en_lookup.get(target) or ''
    # If pt is empty, fallback to en_value
    if not value:
        value = en_value
    set_nested(nested_en, target, en_value)
    set_nested(nested_pt, target, value)

# Also preserve any existing nested keys in en_flat that aren't in extracted keys (safe)
for k,v in en_flat.items():
    if '.' in k and k not in mapping.values():
        set_nested(nested_en, k, v)

# Write out
out_en = os.path.join(base, 'frontend', 'locales', 'en-US.json')
out_pt = os.path.join(base, 'frontend', 'locales', 'pt-PT.json')
with open(out_en, 'w', encoding='utf-8') as f:
    json.dump(nested_en, f, indent=2, ensure_ascii=False)
with open(out_pt, 'w', encoding='utf-8') as f:
    json.dump(nested_pt, f, indent=2, ensure_ascii=False)

print(f'Wrote nested locales to {out_en} and {out_pt}')
