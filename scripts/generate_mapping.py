import json, os

in_path = os.path.join(os.getcwd(), 'scripts', 'extract-translation-keys.txt')
out_path = os.path.join(os.getcwd(), 'scripts', 'proposed-translation-mapping.json')

with open(in_path, 'r', encoding='utf-8') as f:
    keys = [line.strip() for line in f if line.strip()]

mapping = {}

def ns_for(key):
    k = key.lower()
    # Order matters: more specific first
    if 'admin' in k or k.startswith('audit') or 'analytics' in k:
        return 'admin'
    if 'theme' in k or 'accent' in k or 'color' in k or 'preview_mode' in k:
        return 'theme'
    if 'monitor' in k or 'server' in k or 'uptime' in k or 'health' in k or 'load_' in k:
        return 'monitoring'
    if 'chat' in k or 'message' in k or 'private' in k or 'pin_message' in k or 'welcome_universus_chat' in k or 'send'==k or 'type_message'==k:
        return 'chat'
    if 'purchase' in k or 'dark_matter' in k or 'shop' in k or 'item' in k or 'purchase_item' in k:
        return 'shop'
    if 'user' in k or 'username' in k or 'users' in k or 'ban' in k or 'block' in k or 'banned' in k or k.startswith('status_') or 'officers' in k:
        return 'user'
    if 'bot' in k or 'bots' in k or 'bulk_create' in k or 'process_' in k or 'save_bot' in k:
        return 'bots'
    if 'schedule' in k or 'schedules' in k or 'interval' in k or k.endswith('_run') or 'create_schedule' in k:
        return 'schedules'
    if 'event' in k or 'events' in k or 'total_events' in k:
        return 'events'
    if 'resource' in k or 'resources' in k or 'my_fleets' in k or 'fleet' in k or 'officers' in k:
        return 'game'
    if 'analytics' in k:
        return 'analytics'
    # Default to ui
    return 'ui'

for key in keys:
    ns = ns_for(key)
    # If key already contains a dot, leave as-is
    if '.' in key:
        new = key
    else:
        new = f"{ns}.{key}"
    mapping[key] = new

with open(out_path, 'w', encoding='utf-8') as f:
    json.dump(mapping, f, indent=2, ensure_ascii=False)

print(f"Wrote mapping for {len(mapping)} keys to {out_path}")
