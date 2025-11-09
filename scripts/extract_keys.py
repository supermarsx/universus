import re, os, sys

root = os.path.join(os.getcwd(), 'frontend', 'views')
keys = set()
pattern_pipe = re.compile(r"['\"]([A-Za-z0-9_.-]+)['\"]\s*\|\s*t")
pattern_func = re.compile(r"t\(\s*['\"]([A-Za-z0-9_.-]+)['\"]\s*\)")

for dirpath, _, filenames in os.walk(root):
    for fn in filenames:
        if fn.endswith('.njk'):
            path = os.path.join(dirpath, fn)
            try:
                with open(path, 'r', encoding='utf-8') as f:
                    text = f.read()
            except Exception as e:
                # skip unreadable files
                continue
            keys.update(pattern_pipe.findall(text))
            keys.update(pattern_func.findall(text))

out_dir = os.path.join(os.getcwd(), 'scripts')
if not os.path.isdir(out_dir):
    os.makedirs(out_dir, exist_ok=True)

out_path = os.path.join(out_dir, 'extract-translation-keys.txt')
keys_list = sorted(keys)
with open(out_path, 'w', encoding='utf-8') as f:
    for k in keys_list:
        f.write(k + '\n')

print(f"Wrote {len(keys_list)} keys to {out_path}")
