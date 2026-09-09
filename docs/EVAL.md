# Measuring search, instead of forming an impression of it

`samong eval` scores search against questions somebody actually asked. It exists
because the roadmap wants a similarity floor for semantic search, and says the
threshold "has to be measured against real vaults, not guessed" — and because
every ranking change until now has been judged by trying a few queries and
seeing how it felt.

```sh
samong eval questions.toml            # in a vault
samong eval questions.toml --at 10    # hit@10 instead of hit@5
samong eval questions.toml --vault work
```

## The file

```toml
[[question]]
ask = "ทำไม tomcat ต่อ db ไม่ได้"
answers = ["ops/tomcat-jdbc.md"]

[[question]]
ask = "jdbc driver not found"
answers = ["ops/tomcat-jdbc.md"]      # same note, asked in the other language

[[question]]
ask = "how do we rotate the signing key"
answers = []                          # nothing in this vault answers it
```

`answers` are vault-relative paths, exactly as `samong list` prints them. Several
are allowed: a question with two notes that answer it counts as found when either
one comes back, scored at whichever placed better.

An answer key that names no note in the vault is an **error**, not a miss. A typo
would otherwise look exactly like a search that cannot find the note: the question
scores as a miss, the report says ranking got worse, and ranking is fine. A
measurement that can be wrong in a direction nobody notices is worse than no
measurement.

## What it prints

```
42 questions — 37 the vault should answer, 5 it should not
hit@1        24/37  (65%)
hit@5        31/37  (84%)
MRR          0.712
answered anyway  3/5  (60%) — questions with no answer in the vault that still returned something

12 to look at:
  "how do I roll back a bad deploy"
    wanted: ops/rollback.md
    got:    ops/deploy.md, ops/pipeline.md
```

The misses are printed, not just counted. "hit@5 is 84%" tells you to try
something; "these eleven questions missed, and here is what came back instead"
tells you what.

**hit@k** — the correct note in the top k. **MRR** — mean of 1/rank, so moving the
right answer from third to first counts, where hit@5 would not notice.
**answered anyway** — the questions with no answer that got one regardless.

That last number is the point of writing unanswerable questions down. A ranking
change that improves every other number while making search confidently answer
questions it cannot answer has made things worse, and it is the failure that
misleads an agent reading through MCP. So it is counted separately rather than
averaged into the rest. When the similarity floor lands, these two move against
each other: a floor set too low leaves *answered anyway* high, one set too high
shows up as hit@k falling. Neither can be read as progress alone.

MRR is taken over the answerable questions only. Folding the others in as zeroes
would make a vault score worse the more honest gaps its question set admitted to
— and the set would quietly stop admitting to them.

## Writing a set that measures anything

The questions have to be real and the vault has to be yours. A set written by
reading the notes and inventing questions that obviously match them measures the
questions, not the search: it will score near-perfect on the day it is written and
stay there through changes that make search worse.

So take them from where questions actually happen — what you searched for last
week, what a colleague asked in chat, what an agent asked through MCP — and write
down the awkward ones especially:

- the half-remembered phrase rather than the note's title
- an English question about a Thai note, and the reverse
- the acronym or project codename nobody spells out
- a question whose answer moved into a different note since
- questions with no answer at all, roughly one in five

A hundred is plenty; thirty is enough to notice a regression. Keep the file with
the vault it scores — the answers are paths into that vault, so it is only
meaningful there.

## What it does not do

It scores exactly what `samong search` returns for that build. A binary compiled
without the `semantic` feature scores words-only ranking, which is what the
published releases do; the same set against a `--features semantic` build is what
says whether embeddings earned their 465 MB. Comparing two runs is only fair when
the vault, the question set and the build all match.

It says nothing about speed. `hit@5` going up while a search takes four seconds is
not an improvement anybody asked for, and this tool will not tell you that
happened.
