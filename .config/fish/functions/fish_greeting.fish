function fish_greeting
    echo 🪿 Goose says `honk honk`, 🐶 dog says `woof woof`, 🦇 bat says `echo echo`, 🧟 programmer says `I wanna die`.
    echo Stupid jokes over.
    echo ""
    echo -n "It's "; set_color -o cyan; echo -n (date +%A); set_color normal; echo -n ". The time is "; set_color -o magenta; echo -n (date +%H:%M ); set_color normal;
    echo ""
    echo You are on $hostname
end
