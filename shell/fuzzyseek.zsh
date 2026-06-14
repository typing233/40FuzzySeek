#!/usr/bin/env zsh
# FuzzySeek shell integration for Zsh
# Source this file or eval "$(fuzzyseek --shell-integration zsh)"

# Guard against double-sourcing
[[ -n "$__FUZZYSEEK_ZSH_LOADED" ]] && return
__FUZZYSEEK_ZSH_LOADED=1

# --- Configuration ---
: "${FUZZYSEEK_CMD:=fuzzyseek}"
: "${FUZZYSEEK_DEFAULT_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_COMMAND:=find . -path '*/\\.*' -prune -o -type f -print -o -type l -print 2>/dev/null | sed 's|^\\./||'}"
: "${FUZZYSEEK_CTRL_R_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_OPTS:=}"
: "${FUZZYSEEK_ALT_C_COMMAND:=find . -path '*/\\.*' -prune -o -type d -print 2>/dev/null | sed 's|^\\./||'}"

# --- Helper ---
__fuzzyseek_cmd() {
  command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS "$@" </dev/tty 2>/dev/tty
}

# --- Ctrl+R: History search ---
fuzzyseek-history-widget() {
  local output
  output=$(
    fc -rl 1 | sed 's/^ *[0-9]* *//' |
    __fuzzyseek_cmd --query "$LBUFFER" $FUZZYSEEK_CTRL_R_OPTS
  )
  local ret=$?
  if [[ $ret -eq 0 && -n "$output" ]]; then
    LBUFFER="$output"
    RBUFFER=""
  fi
  zle reset-prompt
}
zle -N fuzzyseek-history-widget

# --- Ctrl+T: File search ---
fuzzyseek-file-widget() {
  local output
  output=$(
    eval "$FUZZYSEEK_CTRL_T_COMMAND" |
    __fuzzyseek_cmd --multi $FUZZYSEEK_CTRL_T_OPTS
  )
  local ret=$?
  if [[ $ret -eq 0 && -n "$output" ]]; then
    local selected=""
    while IFS= read -r item; do
      if [[ "$item" == *[[:space:]]* ]]; then
        selected+="\"${item}\" "
      else
        selected+="${item} "
      fi
    done <<< "$output"
    selected="${selected% }"
    LBUFFER+="$selected"
  fi
  zle reset-prompt
}
zle -N fuzzyseek-file-widget

# --- Alt+C: cd to directory ---
fuzzyseek-cd-widget() {
  local output
  output=$(
    eval "$FUZZYSEEK_ALT_C_COMMAND" |
    __fuzzyseek_cmd $FUZZYSEEK_DEFAULT_OPTS
  )
  if [[ -n "$output" ]]; then
    cd -- "$output"
    zle accept-line
  else
    zle reset-prompt
  fi
}
zle -N fuzzyseek-cd-widget

# --- ** expansion ---
fuzzyseek-completion() {
  local tokens=(${(z)LBUFFER})
  local cur="${tokens[-1]}"

  if [[ "$cur" == *"**"* ]]; then
    local prefix="${cur%%\*\**}"
    local suffix="${cur#*\*\*}"
    local dir="${prefix:-.}"

    local output
    output=$(
      find "$dir" -name '.*' -prune -o -print 2>/dev/null |
      sed "s|^${dir}/||" |
      __fuzzyseek_cmd --query "$suffix"
    )

    if [[ -n "$output" ]]; then
      if [[ -z "$prefix" || "$prefix" == "./" ]]; then
        LBUFFER="${LBUFFER%${cur}}${output}"
      else
        LBUFFER="${LBUFFER%${cur}}${prefix}${output}"
      fi
    fi
    zle reset-prompt
  else
    zle expand-or-complete
  fi
}
zle -N fuzzyseek-completion

# --- Bind keys ---
bindkey '^R' fuzzyseek-history-widget
bindkey '^T' fuzzyseek-file-widget
bindkey '\ec' fuzzyseek-cd-widget
bindkey '**' fuzzyseek-completion 2>/dev/null || bindkey '^I' fuzzyseek-completion
