function ssh
    if test "$TERM" = xterm-kitty; and type -q kitty
       kitty +kitten 'ssh' $argv
    else
       command ssh $argv
    end
end
