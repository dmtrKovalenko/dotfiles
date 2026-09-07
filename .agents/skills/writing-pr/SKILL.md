---
name: Writing a PR body/title
description: Use when writing or editing a pull request title or body, or a git commit message.
---

everything below applies equally to commit messages (subject = title, body = body).
utility commits get a subject line only, no body - e.g. fixing ci, fixing tests, formatting, bumping deps, iterating on review feedback, wip. dont narrate what you did or why, the subject is enough.

dont write essays, dont include that you ran tests. rather, write a concise body. focus on mermaid codeblock diagrams, code samples/snippets (this can be internals, or even sample usage). use bullet points for the text you do write. 'validation/i ran tests' is not needed
for visual changes (either directly or indirectly) show a table of before and after with uploaded images/videos.
for benchmarks, always show tables of before/after (baseline from target branch, candidate from the PR)
dont at intermidate PR details - e.g. if we reduced PR size from +6k lines to +1k lines, dont even mention it lol. if we refactored from one commit to another it doesnt matter. only the final aggregate squash merge commit is what matters for commentary
for truely impressive, difficult, or high risk/wide scoped changes you might write the body like a technical blog (again with context, storytelling, code samples/before/after etc diagrams, images whatever.
feel free to use code refs
never add Co-Authored-By trailers or any AI attribution ("Generated with ...") to commits or PR bodies
