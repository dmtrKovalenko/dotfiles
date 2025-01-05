function ripsed
  set search $argv[1]
  set replace $argv[2]

  echo "Results preview"
  if rg -p "$search" -r "$replace" $argv[3..-1] | less -RFX 
    read -p 'set_color -o green; echo "Replace all (Y/N)?";' answer
  else 
    echo "rg failed"
    exit 1
  end

  if test "$answer" = "Y" -o "$answer" = "y"
    if rg $search --files-with-matches $argv[3..-1] | xargs sed -i "" "s/$search/$replace/g"
      echo (set_color -o green) "Replaced all occurrences ✅" (set_color normal)
    else
      echo (set_color -o red) "Replacement failed!" (set_color normal)
    end
  else
    echo "No changes were made."
  end
end
