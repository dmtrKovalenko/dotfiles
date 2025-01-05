function fbr --wraps='git checkout $(git branch | fzf | xargs)' --wraps='git pull origin $(git branch | fzf | xargs)' --description 'alias fbr=git checkout $(git branch | fzf | xargs)'
  git checkout $(git branch | fzf | xargs) $argv
        
end
