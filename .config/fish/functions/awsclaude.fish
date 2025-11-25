function awsclaude
    awslogin

    set -x DISABLE_TELEMETRY 1
    set -x AWS_REGION "us-west-2"
    set -x CLAUDE_CODE_USE_BEDROCK 1

    claude --model global.anthropic.claude-opus-4-5-20251101-v1:0
end
