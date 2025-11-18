function awsclaude
    set -x DISABLE_TELEMETRY 1
    set -x AWS_REGION "us-west-2"
    set -x CLAUDE_CODE_USE_BEDROCK 1

    if not set -q SSH_CONNECTION
        aws sso login --profile dev
    else
        # have to open the browser manually over local shell
        aws sso login --profile dev --no-browser --use-device-code
    end
    eval "$(aws configure export-credentials --profile dev --format env)" && \
    claude --model us.anthropic.claude-sonnet-4-5-20250929-v1:0
end
