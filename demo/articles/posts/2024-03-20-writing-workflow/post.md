---
title: "A Writing Workflow That Actually Sticks"
slug: "writing-workflow"
date: "2024-03-20T09:15:00Z"
tags:
  - writing
  - productivity
abstract: "The system I use to go from a vague idea to a published post without losing momentum along the way."
language: en
attached: []
---

I have tried a lot of writing systems. Most of them collapsed within a few weeks — not because the system was bad, but because it had too many steps between "I have an idea" and "this is published."

What follows is what I actually use, which is considerably simpler.

## The rule

There is one rule: every idea gets a file the moment it appears. Not a note, not a bookmark, not a mental note — a file, with a date and a slug, in the drafts folder.

Most of those files stay empty for weeks. Some stay empty forever. A few get written. The point is not to force yourself to finish everything; it is to not lose anything.

## The folder structure

```
drafts/
  2024-03-20-writing-workflow/
    post.md
  2024-03-15-unused-idea/
    post.md
  2024-02-28-another-thing/
    post.md
```

When a draft is ready, I move it to `posts/`. That is the entire publishing workflow.

## Writing the draft

I write in Markdown. Not because Markdown is great — it is fine — but because plain text files do not rot. They are readable in any editor, on any system, in ten years.

The front matter at the top takes thirty seconds to fill in:

```yaml
---
title: "A Writing Workflow That Actually Sticks"
slug: "writing-workflow"
date: "2024-03-20T09:15:00Z"
tags:
  - writing
---
```

Then I write. I do not edit on the first pass. Editing on the first pass is how drafts stay drafts.

## Editing

I let a draft sit for at least a day before editing. This is not a productivity tip — it is just that I cannot see what is wrong with something I wrote an hour ago. Distance is the only tool that works.

When I come back to edit, I read it aloud. If I stumble over a sentence, the sentence is wrong. If I skip over a paragraph, the paragraph can probably be cut.

## Publishing

```
$ mv drafts/writing-workflow posts/
$ bckt render
```

Done.

## What this does not solve

It does not solve the problem of having nothing to say. If you are not reading, talking to people, or paying attention to the world, no workflow will generate ideas for you.

The system only handles what happens after you have something to say. Everything before that is just living.
