function fish_prompt
  set -l last_status $status

  if not set -q __fish_prompt_char
    switch (id -u)
      case 0
        set -g __fish_prompt_char '#'
      case '*'
        set -g __fish_prompt_char '>'
    end
  end

  # Use cached colors from config.fish
  if test $last_status -eq 0
    echo -n -s $_prompt_success_color '⋊> ' $_prompt_normal
  else
    echo -n -s $_prompt_status_color '⋊> ' $_prompt_normal
  end

  echo -n -s $_prompt_cwd_color (prompt_pwd) $_prompt_normal ' '
end
