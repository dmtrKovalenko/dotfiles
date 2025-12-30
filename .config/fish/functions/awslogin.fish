function awslogin
    set -l profile $argv[1]
    
    # Default to 'dev' if no profile is provided
    if not set -q profile[1]
        set profile dev
    end
    
    if not set -q SSH_CONNECTION
        aws sso login --profile $profile
    else
        # have to open the browser manually over local shell
        aws sso login --profile $profile --no-browser --use-device-code
    end
    eval "$(aws configure export-credentials --profile $profile --format env)"
end

