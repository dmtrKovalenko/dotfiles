---
name: cook-stack
description: Only for use when explicitly requested by the user.
---

Make a series of stacked PRs from the current work. Each PR in the chain should be able to land
safely one at a time so that reviews and reverts are smoother.

First, discuss with the user how to split the work up. Consult any repository-specific change size
guidance applicable. Default to preferring very small PRs of <500 lines added.

Use GitHub's Stacked PRs feature when creating the stack, then use `$writing-pr` for each one. Use the
[`writing-pr`](../writing-pr/SKILL.md) skill for PR titles and descriptions. Have separate subagents
monitor each PR in the stack.

## Stack ordering

Where possible, split work into independently landable streams, and help the user understand the
dependency graph.

When there are legitimate inter-PR dependencies, try to put cosmetic/shim/prefactor work earliest so
that semantic/functional changes can be reviewed as smaller diffs later in the stack.

## Events

How to handle different situations while managing the PR stack.

### All PRs are created

Update all PR bodies with cross-links to the rest of the stack. Bold the current PR's entry and
append ⬅️ to it.

### Updates are pushed to a PR

Notify subagents monitoring dependent PRs to use `$refresh-branch` and to push the results.

### PR is merged or closed

Treat this as an update to the PR and also retire the subagent who was monitoring the merged/closed
PR.

If there are later PRs in the stack, `$refresh-branch` them.
