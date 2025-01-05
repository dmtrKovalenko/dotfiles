function plbr --wraps='git pull origin $(git branch | fzf | xargs)' --description 'alias plbr=git pull origin $(git branch | fzf | xargs)'
  git pull origin $(git branch | fzf | xargs) $argv
        
end
