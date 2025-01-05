function imfzf
  EXTERNAL_COLUMNS=$COLUMNS \
  fzf $argv \
  --preview='kitten icat --clear --transfer-mode=memory --scale-up --place="$COLUMNS"x"$LINES"@(math $EXTERNAL_COLUMNS-$COLUMNS)x0 --align center --stdin=no {} > /dev/tty' \
  --preview-window "right,50%,border-left"
end
