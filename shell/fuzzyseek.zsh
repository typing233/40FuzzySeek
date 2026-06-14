#!/usr/bin/env zsh
# FuzzySeek shell integration for Zsh
# Source this file or eval "$(fuzzyseek --shell-integration zsh)"

# Guard against double-sourcing
[[ -n "$__FUZZYSEEK_ZSH_LOADED" ]] && return
__FUZZYSEEK_ZSH_LOADED=1

# --- Configuration ---
: "${FUZZYSEEK_CMD:=fuzzyseek}"
: "${FUZZYSEEK_DEFAULT_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_COMMAND:=find . -path '*/\.*' -prune -o -type f -print -o -type l -print 2>/dev/null | sed 's|^\./||'}"
: "${FUZZYSEEK_CTRL_R_OPTS:=}"
: "${FUZZYSEEK_CTRL_T_OPTS:=}"
: "${FUZZYSEEK_ALT_C_COMMAND:=find . -path '*/\.*' -prune -o -type d -print 2>/dev/null | sed 's|^\./||'}"

# --- Helper: run fuzzyseek with TUI on /dev/tty ---
# Input is piped in from calling function; stderr goes to /dev/tty for TUI rendering.
# stdin is redirected from /dev/tty so fuzzyseek can read keyboard input.
__fuzzyseek_cmd() {
  command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS "$@" 2>/dev/tty </dev/tty
}

# --- Quote a path for safe insertion into the command line ---
__fuzzyseek_quote() {
  local item="$1"
  if [[ "$item" == *[[:space:]]* || "$item" == *\\* || "$item" == *\'* ||
        "$item" == *\"* || "$item" == *\$* || "$item" == *\`* ||
        "$item" == *\!* || "$item" == *\#* || "$item" == *\&* ||
        "$item" == *\|* || "$item" == *\;* || "$item" == *\(* ||
        "$item" == *\)* || "$item" == *\{* || "$item" == *\}* ||
        "$item" == *\[* || "$item" == *\]* || "$item" == *\<* ||
        "$item" == *\>* || "$item" == *\~* || "$item" == *\** ||
        "$item" == *\?* ]]; then
    # Use ${(q)} for proper zsh quoting
    print -r -- "${(q)item}"
  else
    print -r -- "$item"
  fi
}

# --- Ctrl+R: History search ---
fuzzyseek-history-widget() {
  local output
  # fc -rl 1 lists history; pipe to fuzzyseek; stdin from /dev/tty for keyboard
  output=$(
    fc -rl 1 | sed 's/^ *[0-9]* *//' |
    command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS --query "$LBUFFER" $FUZZYSEEK_CTRL_R_OPTS 2>/dev/tty </dev/tty
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
    command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS --multi $FUZZYSEEK_CTRL_T_OPTS 2>/dev/tty </dev/tty
  )
  local ret=$?
  if [[ $ret -eq 0 && -n "$output" ]]; then
    local selected=""
    local item
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
    command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS 2>/dev/tty </dev/tty
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
  if [[ ${#tokens} -eq 0 ]]; then
    zle expand-or-complete
    return
  fi
  local cur="${tokens[-1]}"

  if [[ "$cur" == *"**"* ]]; then
    local prefix="${cur%%\*\**}"
    local suffix="${cur#*\*\*}"
    local dir="${prefix:-.}"
    local find_dir="$dir"
    [[ "$find_dir" == */ && "$find_dir" != "/" ]] && find_dir="${find_dir%/}"

    local output
    output=$(
      find "$find_dir" -name '.*' -prune -o -print 2>/dev/null |
      sed "s|^${find_dir}/||" |
      command "$FUZZYSEEK_CMD" $FUZZYSEEK_DEFAULT_OPTS --query "$suffix" 2>/dev/tty </dev/tty
    )

    if [[ -n "$output" ]]; then
      local result
      if [[ -z "$prefix" || "$prefix" == "./" ]]; then
        result="$output"
      else
        result="${prefix}${output}"
      fi
      local quoted
      quoted=$(__fuzzyseek_quote "$result")
      LBUFFER="${LBUFFER%${cur}}${quoted}"
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
bindkey '^^' fuzzyseek-completion 2>/dev/null
# Tab for ** trigger only when ** is present (otherwise normal completion)
bindkey '^I' fuzzyseek-completion
