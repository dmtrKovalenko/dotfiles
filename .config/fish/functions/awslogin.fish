function awslogin
    if not set -q SSH_CONNECTION
        aws sso login --profile dev
    else
        # have to open the browser manually over local shell
        aws sso login --profile dev --no-browser --use-device-code
    end
    eval "$(aws configure export-credentials --profile dev --format env)"
end
