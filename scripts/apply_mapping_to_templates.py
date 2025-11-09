import json, os, re

base = os.getcwd()
map_path = os.path.join(base, 'scripts', 'proposed-translation-mapping.json')
views_root = os.path.join(base, 'frontend', 'views')

with open(map_path, 'r', encoding='utf-8') as f:
    mapping = json.load(f)

# Build regex patterns
# Matches 'key' | t  or "key" | t
pattern_pipe = re.compile(r"(['\"])([A-Za-z0-9_.-]+)\1(\s*\|\s*t)" )
# Matches t('key') or t("key")
pattern_func = re.compile(r"t\(\s*(['\"])([A-Za-z0-9_.-]+)\1\s*\)")

replacements = 0
files_changed = 0

for dirpath, _, filenames in os.walk(views_root):
    for fn in filenames:
        if not fn.endswith('.njk'):
            continue
        path = os.path.join(dirpath, fn)
        with open(path, 'r', encoding='utf-8') as f:
            text = f.read()
        orig = text

        def repl_pipe(m):
            key = m.group(2)
            mapped = mapping.get(key)
            if not mapped:
                return m.group(0)
            return "'{}'{}".format(mapped, m.group(3))

        def repl_func(m):
            key = m.group(2)
            mapped = mapping.get(key)
            if not mapped:
                return m.group(0)
            return "t('{}')".format(mapped)

        text = pattern_pipe.sub(repl_pipe, text)
        text = pattern_func.sub(repl_func, text)

        if text != orig:
            with open(path, 'w', encoding='utf-8') as f:
                f.write(text)
            files_changed += 1
            # rough count of replacements
            replacements += sum(1 for _ in re.finditer(pattern_pipe, orig)) + sum(1 for _ in re.finditer(pattern_func, orig))

print(f"Updated {files_changed} template files; approx {replacements} replacements")
