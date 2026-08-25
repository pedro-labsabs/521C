# shellcheck shell=bash
# Source this file to put repo-local portable tools (.tools/bin) on PATH
# for the current shell only. It never modifies global shell configuration.
#   source scripts/env.sh
_521c_root="$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." && pwd)"
case ":$PATH:" in
  *":$_521c_root/.tools/bin:"*) ;;
  *) PATH="$_521c_root/.tools/bin:$PATH" ;;
esac
export PATH
unset _521c_root
