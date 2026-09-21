# GPG needs to know the controlling terminal to prompt for passphrases.
# Without this, gpg fails with "Inappropriate ioctl for device".
# Only meaningful in an interactive shell; `tty` has nothing to report otherwise.
if status is-interactive
    set -gx GPG_TTY (tty)
end
