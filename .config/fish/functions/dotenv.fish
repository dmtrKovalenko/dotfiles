function dotenv
  for line in (cat $argv | grep -v '^#')
    echo $line
    set item (string match -r '^(?:export\s+)(?<envvar>\w{1,200})="(?<envval>\N{1,10000})"' $line)
    
    if test $envvar = "PATH"
      fish_add_path $PATH
    else
      set -gx $envvar $envval
    end

    echo "Exported key $envvar"
  end
end
