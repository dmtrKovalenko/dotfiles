---
name: Writing a PR body/title or commit message
description: Use when writing or editing a pull request title or body, or writing a git commit message's body.
---

Always use conventional commit tags + scope of a change as a PR title / commit message first line. Title and commit first line should always be a single sentence in AS-STE100 format that is *CLEARLY* describing the change or if it was a fix of *WHAT* you have fixed.

everything below applies equally to commit messages (subject = title, body = body).
utility commits get a subject line only, no body - e.g. fixing ci, fixing tests, formatting, bumping deps, iterating on review feedback, wip. dont narrate what you did or why, the subject is enough.

The structure of the PR. A single summary of the change, no more than 3 sentences in ASD-STE100 format, ideally one or two. This is where everything belongs. Focus on this part and create a clear one term-per-sentence description of a change. After that add separaor (---) and you can give more evidence. Evidence includes mermaid graphs, code samples, github style >[!TIP] or >[!IMPORTANT] or >[!NOTE] blocks, if there are benchmarks or bulky change always include before/after table in the desktopion after the line. Do not write long paragraph of text, noone is reading those, make a diagram, image, table, or short paragraphs of waht you have to say

for truely impressive, difficult, or high risk/wide scoped changes you might write the body like a technical blog (again with context, storytelling, code samples/before/after etc diagrams, images whatever.

**IMPORTANT** never add Co-Authored-By trailers or any AI attribution ("Generated with ...") to commits or PR bodies
