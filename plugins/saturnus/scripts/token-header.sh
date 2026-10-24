#!/bin/sh
# The Authorization header for the saturnus desktop app's MCP endpoint, for
# Claude Code's headersHelper (plugins/saturnus/.mcp.json). Reads the
# per-user token file the desktop app creates when AI access is turned on
# (crates/saturnus-drive/src/loopback/token.rs, default_path and resolve):
# $SATURNUS_TOKEN_FILE, else %LOCALAPPDATA%\saturnus\control-token on
# Windows, else $XDG_CONFIG_HOME/saturnus/control-token, else
# ~/.config/saturnus/control-token. Prints {"Authorization": "Bearer …"} to
# stdout and nothing else; the token is never written anywhere else.
# Claude Code removes variables named like credentials (SATURNUS_TOKEN_FILE
# among them) before it runs a plugin's helper, so there the default path
# applies; the variable counts when the script runs by hand.
set -eu

if [ -n "${SATURNUS_TOKEN_FILE:-}" ]; then
  file=$SATURNUS_TOKEN_FILE
else
  case "$(uname -s 2>/dev/null || echo unknown)" in
    MINGW* | MSYS* | CYGWIN*) windows=1 ;;
    *) windows=0 ;;
  esac
  if [ "$windows" = 1 ] && [ -n "${LOCALAPPDATA:-}" ]; then
    file="$LOCALAPPDATA/saturnus/control-token"
  else
    case "${XDG_CONFIG_HOME:-}" in
      /*) base=$XDG_CONFIG_HOME ;;
      *) base="${HOME:?HOME is not set}/.config" ;;
    esac
    file="$base/saturnus/control-token"
  fi
fi

if [ ! -r "$file" ]; then
  echo "saturnus: no token file at $file. In the saturnus desktop app, open \"Connect your AI app\" and turn on \"Let AI apps on this computer use the calculators\"." >&2
  exit 1
fi

token=$(tr -d ' \t\r\n' < "$file")
case "$token" in
  *[!0-9a-f]*) ok=0 ;;
  *) ok=1 ;;
esac
if [ "$ok" = 0 ] || [ "${#token}" -ne 64 ]; then
  echo "saturnus: the token file at $file is not a saturnus token (64 hex digits)." >&2
  exit 1
fi

printf '{"Authorization":"Bearer %s"}\n' "$token"
