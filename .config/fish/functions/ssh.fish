function ssh
    if [ "$TERM" = "xterm-kitty" ]
       kitty +kitten 'ssh' $argv
    else
       ssh $argv
    end
end
