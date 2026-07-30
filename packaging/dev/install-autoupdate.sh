#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
hook_path="$repo_root/.git/hooks/post-commit"

"$repo_root/packaging/dev/reinstall-and-restart.sh"

cat > "$hook_path" <<'HOOK'
#!/usr/bin/env bash
set -euo pipefail

repo_root="$(git rev-parse --show-toplevel)"
"$repo_root/packaging/dev/reinstall-and-restart.sh"
HOOK

chmod +x "$hook_path"
echo "Installed post-commit auto-update hook: $hook_path"

