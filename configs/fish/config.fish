# RavenLinux default fish configuration

# Environment
set -gx RAVEN_LINUX 1
set -gx LANG en_US.UTF-8
set -gx LC_ALL en_US.UTF-8

# XDG directories
set -q XDG_CONFIG_HOME; or set -gx XDG_CONFIG_HOME $HOME/.config
set -q XDG_DATA_HOME; or set -gx XDG_DATA_HOME $HOME/.local/share
set -q XDG_CACHE_HOME; or set -gx XDG_CACHE_HOME $HOME/.cache
set -q XDG_STATE_HOME; or set -gx XDG_STATE_HOME $HOME/.local/state

# Path
fish_add_path -g ~/bin ~/.local/bin /usr/local/bin

# Editor
set -q EDITOR; or set -gx EDITOR raven-editor
set -q VISUAL; or set -gx VISUAL raven-editor

# Prompt
function fish_prompt
    echo -n "[$(whoami)@raven-linux]# "
end

# Disable greeting
set -g fish_greeting

# Aliases
alias ls='ls --color=auto'
alias ll='ls -lah'
alias la='ls -A'
alias l='ls -CF'
alias grep='grep --color=auto'
alias ..='cd ..'
alias ...='cd ../..'

# RavenLinux specific
alias rvn='rvn'
alias raven-update='rvn upgrade'

# fzf integration (if installed)
if test -f /usr/share/fzf/key-bindings.fish
    source /usr/share/fzf/key-bindings.fish
end

# Ranger cd integration - use 'rcd' to cd to last ranger directory
function rcd
    set tempfile (mktemp -t ranger_cd.XXXXXX)
    ranger --choosedir=$tempfile $argv
    if test -f $tempfile
        set dir (cat $tempfile)
        if test -d "$dir" -a "$dir" != (pwd)
            cd $dir
        end
    end
    rm -f $tempfile
end

# Fuzzy find file and open in editor
function fe
    set file (fzf --preview 'head -100 {}' 2>/dev/null)
    if test -n "$file"
        $EDITOR $file
    end
end

# Fuzzy cd to directory
function fcd
    set dir (find . -type d 2>/dev/null | fzf)
    if test -n "$dir"
        cd $dir
    end
end

# Load local config if exists
if test -f ~/.config/fish/config.fish.local
    source ~/.config/fish/config.fish.local
end
