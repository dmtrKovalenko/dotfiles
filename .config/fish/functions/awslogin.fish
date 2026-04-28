function awslogin
    set -l profile $argv[1]

    if not set -q profile[1]
        if set -q AWS_PROFILE
            set profile $AWS_PROFILE
        else
            set profile dev
        end
    end

    if not set -q SSH_CONNECTION
        aws sso login --profile $profile
    else
        # have to open the browser manually over local shell
        aws sso login --profile $profile --no-browser --use-device-code
    end

    # Export creds to current shell for tools that ignore AWS_PROFILE/SSO cache.
    # New shells still work via universal AWS_PROFILE + SSO cache in ~/.aws/sso/cache.
    eval "$(aws configure export-credentials --profile $profile --format env)"
end

