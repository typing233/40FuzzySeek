#!/usr/bin/env fish
# FuzzySeek shell integration for Fish
# Source this file or: fuzzyseek --shell-integration fish | source

# Guard against double-sourcing
if set -q __FUZZYSEEK_FISH_LOADED
    return
end
set -g __FUZZYSEEK_FISH_LOADED 1

# --- Configuration ---
set -q FUZZYSEEK_CMD; or set -g FUZZYSEEK_CMD fuzzyseek
set -q FUZZYSEEK_DEFAULT_OPTS; or set -g FUZZYSEEK_DEFAULT_OPTS ""
set -q FUZZYSEEK_CTRL_T_COMMAND; or set -g FUZZYSEEK_CTRL_T_COMMAND "find . -path '*/.*' -prune -o -type f -print -o -type l -print 2>/dev/null | sed 's|^\\./||'"
set -q FUZZYSEEK_ALT_C_COMMAND; or set -g FUZZYSEEK_ALT_C_COMMAND "find . -path '*/.*' -prune -o -type d -print 2>/dev/null | sed 's|^\\./||'"

# --- Helper: properly escape a path for insertion ---
function __fuzzyseek_escape
    # Use fish's built-in string escape for command-line safety
    string escape -- $argv
end

# --- Ctrl+R: History search ---
function __fuzzyseek_history
    set -l query (commandline)
    # Pipe history to fuzzyseek; redirect stderr to tty for rendering,
    # redirect stdin from tty so fuzzyseek gets keyboard input
    set -l output (
        builtin history |
        command $FUZZYSEEK_CMD $FUZZYSEEK_DEFAULT_OPTS --query "$query" 2>/dev/tty </dev/tty
    )
    if test $status -eq 0; and test -n "$output"
        commandline -r -- "$output"
    end
    commandline -f repaint
end

# --- Ctrl+T: File search ---
function __fuzzyseek_file
    set -l output (
        eval $FUZZYSEEK_CTRL_T_COMMAND |
        command $FUZZYSEEK_CMD $FUZZYSEEK_DEFAULT_OPTS --multi 2>/dev/tty </dev/tty
    )
    if test $status -eq 0; and test -n "$output"
        # Properly quote each selected item and join with spaces
        set -l escaped
        for item in $output
            set -a escaped (__fuzzyseek_escape $item)
        end
        commandline -it -- (string join ' ' -- $escaped)
    end
    commandline -f repaint
end

# --- Alt+C: cd to directory ---
function __fuzzyseek_cd
    set -l output (
        eval $FUZZYSEEK_ALT_C_COMMAND |
        command $FUZZYSEEK_CMD $FUZZYSEEK_DEFAULT_OPTS 2>/dev/tty </dev/tty
    )
    if test -n "$output"
        cd -- "$output"
    end
    commandline -f repaint
end

# --- Bind keys ---
bind \cr __fuzzyseek_history
bind \ct __fuzzyseek_file
bind \ec __fuzzyseek_cd

# Also bind in insert mode if vi mode is active
if bind -M insert >/dev/null 2>&1
    bind -M insert \cr __fuzzyseek_history
    bind -M insert \ct __fuzzyseek_file
    bind -M insert \ec __fuzzyseek_cd
end
