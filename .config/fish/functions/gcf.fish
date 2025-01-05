function gcf --wraps='git diff --name-only --diff-filter=U' --description 'alias gcf=git diff --name-only --diff-filter=U'
  git diff --name-only --diff-filter=U $argv
        
end
