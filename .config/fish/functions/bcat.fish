set external_columns $COLUMNS

function bcat --description 'Display a file using icat or bat'
    set file $argv[1]

    # Check if the file is an image
    if string match -qr '.*\.(png|jpg|jpeg|gif|bmp)' $file
        # Use `icat` from `kitty` to display the image
        # kitten icat --clear --transfer-mode=memory --scale-up --place="$COLUMNS"x"$LINES"@(math $EXTERNAL_COLUMNS-$COLUMNS)x0 --align center --stdin=no $file > /dev/tty
        echo $LINES $COLUMNS $external_columns
        kitten icat --clear --transfer-mode=memory --scale-up --place="$COLUMNS"x"$LINES"@(math $external_columns-$COLUMNS)x0 --align center --stdin=no $file > /dev/tty
    else
        # Use `bat` to display the file
        bat --style=numbers --color=always --line-range :300 $file
    end
end
