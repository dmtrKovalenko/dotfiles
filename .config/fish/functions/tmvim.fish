function tmvim --wraps='nvim . -u ~/.config/nvim/terminal.lua' --description 'alias tmvim=nvim . -u ~/.config/nvim/terminal.lua'
  if test -d $argv[1]
    cd $argv[1]
  else
      echo "Given argument is not a directory."
  end 

  set lightsource_dir (realpath ~/dev/lightsource)
  set cwd (pwd)
  if test "$cwd" = "$lightsource_dir"
      nvim . -u ~/.config/nvim/terminal.lua -c "LightSourceSetup"
  else
      nvim . -u ~/.config/nvim/terminal.lua 
  end

end
