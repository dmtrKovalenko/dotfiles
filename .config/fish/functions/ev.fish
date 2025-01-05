if set -q XDG_CONFIG_HOME
    set config_dir $XDG_CONFIG_HOME
else
    set config_dir ~/.config/env
end

set -g DEFAULT_EVPATH $config_dir/env

# Display general usage
function __ev_usage
    echo "\
Usage:
    ev [-q] PATH ...
    ev [-q] -u PATH ...

Options:
    -q            No output
    -u PATH       Unset environment variables in PATH
    -h --help     Show this message

EVPATH=$EVPATH
EVPATH is the directory or list of directories where ev will search.
PATH can be a name of a file/directory in EVPATH or a path to a file/directory.
"
end

function __ev_print_paths
    set r $argv[1]
    set x (string match -r '.*/' $argv[2])
    or set x ''
    if not test -e $x; and test -e $r/$x
        for f in (command ls --color=never -1p $r/$x)
            echo $x$f
        end
    end
end

function __ev_update_completions
    set -q EVPATH ; or set -g EVPATH $DEFAULT_EVPATH

    complete -c ev -x -s h -l help -d 'help'
    complete -c ev -s q -d 'quiet'
    for evdir in $EVPATH
        set parent (string replace -r "^$HOME" "~" -- "$evdir")
        complete -c ev -a "(__ev_print_paths $evdir (commandline -ct))" -d $parent
        complete -c ev -s u -a "(__ev_print_paths $evdir (commandline -ct))" -d $parent
    end
end

function __ev_expand_path
    set -q EVPATH ; or set -g EVPATH $DEFAULT_EVPATH
    set -l path "$argv[1]"
    if test -e "$path"
        echo "$path"
        return 0
    else
        for evdir in $EVPATH
            set -l fp "$evdir/$path"
            if test -e "$fp"
                echo $fp
                return 0
            end
        end
    end
    return 1
end

function __ev_ls
  set -l path $argv[1]
  switch (uname)
    case Darwin
      command ls -1 "$path"
    case '*'
      command ls --color=never -1 "$path"
  end
end


function __ev_handle_path
    set -l quiet "$argv[1]"
    set -l unset "$argv[2]"
    set -l path "$argv[3]"
    if test -d "$path"
        for fn in (__ev_ls $path)
            __ev_handle_path $quiet $unset "$path/$fn"
            or return 1
        end
        return 0
    else if test -f "$path"
        set -l varname (basename $path)
        if string match -qvr '^[a-zA-Z0-9_]+$' "$varname"
            echo "ev: invalid variable name: $varname" >&2
            return 1
        end
        if [ "$quiet" != "yes" ]
            echo $varname
        end
        if [ "$unset" = "yes" ]
            set -e "$varname"
        else if test -x "$path"
            set -gx $varname (eval "$path")
        else
            set -gx $varname (cat "$path")
        end
        return 0
    end
    return 1
end

function ev -d 'Load environment variables from files and directories'
    set -q EVPATH ; or set -g EVPATH $DEFAULT_EVPATH

    if test (count $argv) -lt 1
        __ev_usage >&2
        return 1
    end

    set quiet no
    set unset no
    for opt in $argv
        switch $opt
            case -h --help
                __ev_usage
                return 0
            case -q
                set quiet yes
            case -u
                set unset yes
            case '*'
                set paths $paths $opt
        end
    end

    for path in $paths
        set expanded (__ev_expand_path $path)
        or begin
            echo "ev: $path: No such file or directory" >&2
            return 1
        end
        __ev_handle_path $quiet $unset "$expanded"
        or return 1
    end

end
