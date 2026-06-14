#!/usr/bin/env bash
# FuzzySeek shell integration for Bash
# Source this file or eval "$(fuzzyseek --shell-integration bash)"

# Guard against double-sourcing
[[ -n "$__FUZZYSEEK_BASH_LOADED" ]] && return
__FUZZYSEEK_BASH_LOADED=1

# --- Configuration ---
: "${FUZZYSEEK_CMD:=fuzzyseek}"
: "${FUZZYSEEK_DEFAULT_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_COMMAND:=find . -path '*/\\.*' -prune -o -type f -print -o -type l -print 2>/dev/null | sed 's|^\\./||'}"
: "${FUZZYSEEK_CTRL_R_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_OPTS:=}"
: "${FUZZYSEEK_ALT_C_COMMAND:=find . -path '*/\\.*' -prune -o -type d -print 2>/dev/null | sed 's|^\\./||'}"

# --- Helper: invoke fuzzyseek with TTY ---
__fuzzyseek_cmd() {
  command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS "$@" </dev/tty 2>/dev/tty
}

# --- Ctrl+R: History search ---
__fuzzyseek_history() {
  local output
  output=$(
    HISTTIMEFORMAT= builtin history | command sed 's/^ *[0-9]* *//' |
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
    # Handle multi-select: join with spaces, quoting if needed
    local selected=""
    while IFS= read -r item; do
      if [[ "$item" == *[[:space:]]* ]]; then
        selected+="\"$item\" "
      else
        selected+="$item "
      fi
    done <<< "$output"
    selected="${selected% }"
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
    # Reset prompt
    if [[ -n "$PROMPT_COMMAND" ]]; then
      eval "$PROMPT_COMMAND"
    fi
  fi
}

# --- ** completion trigger ---
__fuzzyseek_completion() {
  local cur="${COMP_WORDS[COMP_CWORD]}"

  if [[ "$cur" == *"**"* ]]; then
    local prefix="${cur%%\*\**}"
    local suffix="${cur#*\*\*}"
    local dir="${prefix:-.}"

    # Strip trailing / from prefix for display
    [[ "$prefix" == */ ]] && prefix="${prefix%/}/"

    local output
    output=$(
      find "$dir" -name '.*' -prune -o -print 2>/dev/null |
      sed "s|^${dir}/||" |
      __fuzzyseek_cmd --query "$suffix"
    )

    if [[ -n "$output" ]]; then
      if [[ "$prefix" == "./" || -z "$prefix" ]]; then
        COMPREPLY=("$output")
      else
        COMPREPLY=("${prefix}${output}")
      fi
    fi
    return 0
  fi
}

# --- Bind keys ---
if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
  bind -x '"\C-r": __fuzzyseek_history'
  bind -x '"\C-t": __fuzzyseek_file_widget'
  bind -x '"\ec": __fuzzyseek_cd'
fi

# Register ** completion for common commands
if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
  complete -D -F __fuzzyseek_completion
fi
