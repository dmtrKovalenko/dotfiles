function awsclaude
    awslogin

    set -x DISABLE_TELEMETRY 1
    set -x AWS_REGION "us-west-2"
    set -x CLAUDE_CODE_USE_BEDROCK 1
    set -x ANTHROPIC_DEFAULT_HAIKU_MODEL "us.anthropic.claude-haiku-4-5-20251001-v1:0"

    claude --model  us.anthropic.claude-opus-4-7  --dangerously-skip-permissions --effort max
end
