function gmc --wraps='git diff --name-only --diff-filter=U' --description 'alias gmc=git diff --name-only --diff-filter=U'
  git diff --name-only --diff-filter=U $argv
        
end
