function awsclaude
    awslogin

    set -x DISABLE_TELEMETRY 1
    set -x AWS_REGION "us-west-2"
    set -x CLAUDE_CODE_USE_BEDROCK 1

    claude --model us.anthropic.claude-opus-4-6-v1 --dangerously-skip-permissions
end
