#!/usr/bin/env bash
# FuzzySeek shell integration for Bash
# Source this file or eval "$(fuzzyseek --shell-integration bash)"

# Guard against double-sourcing
[[ -n "$__FUZZYSEEK_BASH_LOADED" ]] && return
__FUZZYSEEK_BASH_LOADED=1

# --- Configuration ---
: "${FUZZYSEEK_CMD:=fuzzyseek}"
: "${FUZZYSEEK_DEFAULT_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_COMMAND:=find . -path '*/\.*' -prune -o -type f -print -o -type l -print 2>/dev/null | sed 's|^\./||'}"
: "${FUZZYSEEK_CTRL_R_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_OPTS:=}"
: "${FUZZYSEEK_ALT_C_COMMAND:=find . -path '*/\.*' -prune -o -type d -print 2>/dev/null | sed 's|^\./||'}"

# --- Helper: invoke fuzzyseek safely with TTY for both stdin and stderr ---
# fuzzyseek renders TUI on stderr (/dev/tty) and reads keyboard from /dev/tty
# internally (via crossterm use-dev-tty). Candidates arrive via stdin from the pipe.
__fuzzyseek_cmd() {
  command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS "$@" 2>/dev/tty
}

# --- Properly quote a filename for insertion into the command line ---
__fuzzyseek_quote() {
  local item="$1"
  # If it contains any special characters, quote it with single quotes
  if [[ "$item" =~ [[:space:]\\\'\"\$\`\!\#\&\|\;\(\)\{\}\[\]\<\>\~\*\?] ]]; then
    # Escape existing single quotes, then wrap in single quotes
    printf "%s" "'${item//\'/\'\\\'\'}'"
  else
    printf "%s" "$item"
  fi
}

# --- Ctrl+R: History search ---
__fuzzyseek_history() {
  local output
  output=$(
    HISTTIMEFORMAT= builtin history |
    command sed 's/^ *[0-9]* *//' |
    __fuzzyseek_cmd --query "$READLINE_LINE" $FUZZYSEEK_CTRL_R_OPTS
  )
  local ret=$?
  if [[ $ret -eq 0 && -n "$output" ]]; then
    READLINE_LINE="$output"
    READLINE_POINT=${#output}
  fi
}

# --- Ctrl+T: File search ---
__fuzzyseek_file_widget() {
  local output
  output=$(
    eval "$FUZZYSEEK_CTRL_T_COMMAND" |
    __fuzzyseek_cmd --multi $FUZZYSEEK_CTRL_T_OPTS
  )
  local ret=$?
  if [[ $ret -eq 0 && -n "$output" ]]; then
    local selected=""
    while IFS= read -r item; do
      [[ -z "$item" ]] && continue
      local quoted
      quoted=$(__fuzzyseek_quote "$item")
      if [[ -n "$selected" ]]; then
        selected+=" $quoted"
      else
        selected="$quoted"
      fi
    done <<< "$output"
    READLINE_LINE="${READLINE_LINE:0:$READLINE_POINT}${selected}${READLINE_LINE:$READLINE_POINT}"
    READLINE_POINT=$((READLINE_POINT + ${#selected}))
  fi
}

# --- Alt+C: cd to directory ---
__fuzzyseek_cd() {
  local output
  output=$(
    eval "$FUZZYSEEK_ALT_C_COMMAND" |
    __fuzzyseek_cmd $FUZZYSEEK_DEFAULT_OPTS
  )
  if [[ -n "$output" ]]; then
    builtin cd -- "$output" || return
  fi
}

# --- ** completion trigger ---
__fuzzyseek_completion() {
  local cur="${COMP_WORDS[COMP_CWORD]}"

  if [[ "$cur" == *"**"* ]]; then
    local prefix="${cur%%\*\**}"
    local suffix="${cur#*\*\*}"
    local dir="${prefix:-.}"

    # Normalize: strip trailing slash for find but keep for prefix
    local find_dir="$dir"
    [[ "$find_dir" == */ && "$find_dir" != "/" ]] && find_dir="${find_dir%/}"

    local output
    output=$(
      find "$find_dir" -name '.*' -prune -o -print 2>/dev/null |
      sed "s|^${find_dir}/||" |
      __fuzzyseek_cmd --query "$suffix"
    )

    if [[ -n "$output" ]]; then
      local result
      if [[ "$prefix" == "./" || -z "$prefix" ]]; then
        result="$output"
      else
        result="${prefix}${output}"
      fi
      # Properly escape for COMPREPLY
      COMPREPLY=("$(printf '%q' "$result")")
    fi
    return 0
  fi
}

# --- Bind keys (requires Bash 4+) ---
if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
  bind -x '"\C-r": __fuzzyseek_history'
  bind -x '"\C-t": __fuzzyseek_file_widget'
  bind -x '"\ec": __fuzzyseek_cd'
  complete -D -F __fuzzyseek_completion
fi
