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

`answers` are vault-relative paths, exactly as `samong list` prints them — the
same keys `samong search` prints in front of each hit. Not titles: a title is a
display name, and `README.md` and `docs/README.md` share one. Several answers
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

## Choosing a similarity floor

Rank fusion admits the vector store's best candidate whatever its score. The
store always returns *something*, and RRF only reads position, so on a question
the vault cannot really answer an unremarkable match still lands near the top.
A similarity floor drops candidates below a cosine score before the fusion —
`--semantic-floor 0.84` on `samong search` and on `samong eval`.

There is no shipped default, and that is the point: the number depends on the
vault and the model, so it is measured rather than reasoned about.

```sh
samong eval --floors 0.70,0.75,0.80,0.84,0.88,0.92 questions.toml
```

**Start high.** Samong embeds with `intfloat/multilingual-e5-small`, and the e5
family compresses cosine into a narrow band near the top: two passages with
nothing to do with each other still score around 0.75, and a real match around
0.85–0.92. A floor of 0.3 — the number the word "similarity" suggests — sits
below the entire distribution and filters nothing at all.

That failure is silent in the only way that matters: **every row of the sweep
comes back identical**, which reads exactly like "the floor does not change
anything" rather than "these floors are all below the lowest score there is". If
a sweep prints the same numbers on every line, widen the range upward before
concluding the feature is inert. It is the first sweep ever run on this tool that
wasted a round this way.

Each row is the same question set through the same search path, differing only in
the floor, with `none` first — the row every other row has to beat.

## The first sweep, and what it does and does not settle

Run on 2026-09-14 against the vault of a Java/Next.js project: 38 notes, mostly
architecture documentation written in Thai and English together, and 21 questions
its author had actually asked while working on it — 15 the vault answers, 6 it
does not.

```
floor         hit@1      hit@5      MRR  answered anyway
none          11/15      15/15    0.867              6/6
0.70          11/15      15/15    0.867              6/6
0.75          11/15      15/15    0.867              6/6
0.80          11/15      15/15    0.867              6/6
0.84          11/15      15/15    0.867              5/6
0.88          12/15      15/15    0.900              1/6
0.92          15/15      15/15    1.000              1/6
```

The same set with the `semantic` feature absent — lexical retrieval alone —
scores **15/15, MRR 1.000, answered anyway 1/6**.

Read those two together, because the second is the finding. Semantic retrieval
with no floor **took four correct notes off the top spot** and made the vault
answer every one of the six questions it has no answer to, up from one. This is
the defect the roadmap had described in the abstract since the first public
release: RRF reads position and never score, the vector store always returns
something, so its best candidate enters the fusion however unlike the question it
is. The cost of that had never been a number before.

It is also why **no default ships from this table.** Lexical alone already scores
the maximum on this set, so no floor can beat it — only approach it, and the 0.92
row reaching exactly the lexical numbers is the tell: that floor is high enough to
discard essentially every semantic candidate, which is the feature switched off by
another name. A set where semantic retrieval never once helps measures only half
the trade: it locates where the harm stops and says nothing about where the
benefit lives. Picking the top of that range and calling it measured would be a
guess wearing the costume of a measurement, which is the one thing this file
exists to argue against.

What the set is missing is questions lexical retrieval loses: asked in the other
language from the note that answers them, in the words of the symptom rather than
the words of the document, or with an abbreviation nobody wrote down. Those are
the questions a floor has to be careful not to throw away, and until a set
contains some, the floor stays unset.

Read the columns against each other, not one at a time. A floor that lifts hit@1
while lifting **answered anyway** has made search more confidently wrong, which is
worse than leaving it alone — that column is why a set needs questions with no
answer in it. The floor worth keeping is the one that drops the last column
without moving the first three, and where the rows either side of it say roughly
the same thing: a number that only works at exactly one value is a number fitted
to this question set rather than to the vault.

Both flags need a build with `--features semantic` and a vault that has been
through `samong embed`. Without them the vector store is empty, the floor has
nothing to filter, and every row comes back identical — the sweep says so rather
than letting the table imply the floor did nothing.
