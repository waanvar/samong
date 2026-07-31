#!/usr/bin/env bash
# A demo vault for the README screenshots.
#
# Scripted rather than hand-made so the images can be regenerated when the UI
# changes — which is how they went stale in the first place. The notes are short
# but genuinely interlinked and foldered, because the graph's whole claim is that
# it shows the shape your paths and links already describe; a flat list of
# disconnected notes would photograph as noise.
#
# Two Thai notes are in here on purpose: Thai segmentation is the thing Samong
# does that others do not, and the README should show it rather than assert it.
set -eu
V="${1:?usage: demovault.sh <dir>}"
rm -rf "$V"
mkdir -p "$V/decisions" "$V/runbooks" "$V/notes"

w() { mkdir -p "$(dirname "$V/$1")"; printf '%s\n' "$2" > "$V/$1"; }

w "README.md" '# Payments platform

Working notes for the billing service. Start at [[Architecture]], or
[[Rate limiting]] if that is why you are here.

- [[Architecture]]
- [[Runbook — deploy]]
- [[Glossary]]'

w "Architecture.md" '# Architecture

Three services behind one gateway. [[Rate limiting]] happens at the edge;
[[Idempotency]] is the reason retries are safe.

See [[ADR-002 Postgres over DynamoDB]] and [[ADR-005 Outbox pattern]].'

w "Rate limiting.md" '# Rate limiting

Token bucket per API key, 100 requests a second with a burst of 200. The bucket
lives in Redis so every gateway pod agrees.

Returns 429 with `Retry-After`. Clients that ignore it get [[Idempotency]] for
free, which is not the same as being polite.

Related: [[Architecture]], [[Runbook — incident]].'

w "Idempotency.md" '# Idempotency

Every mutating endpoint takes an `Idempotency-Key`. We store the first response
for 24 hours and replay it. This is what makes [[Rate limiting]] retries and
[[ADR-005 Outbox pattern]] safe together.'

w "Glossary.md" '# Glossary

- **Bucket** — see [[Rate limiting]]
- **Outbox** — see [[ADR-005 Outbox pattern]]
- **Tenant** — one customer, one schema. See [[Architecture]].'

w "decisions/ADR-002 Postgres over DynamoDB.md" '# ADR-002 Postgres over DynamoDB

Accepted. We need transactions across ledger rows, and the team already knows
Postgres. Revisit if write throughput passes 5k/s.

Consequence: [[ADR-005 Outbox pattern]] instead of streams.'

w "decisions/ADR-005 Outbox pattern.md" '# ADR-005 Outbox pattern

Accepted. Writes and their events land in one transaction; a poller publishes.
Follows from [[ADR-002 Postgres over DynamoDB]] and makes [[Idempotency]]
tractable on the consumer side.'

w "decisions/ADR-011 Retire the v1 API.md" '# ADR-011 Retire the v1 API

Proposed. v1 has no [[Idempotency]] and its own [[Rate limiting]] rules, which
is two problems we keep paying for.'

w "decisions/ADR-014 Self-host the queue.md" '# ADR-014 Self-host the queue

Rejected. Operational cost was not worth the saving. See [[ADR-005 Outbox pattern]].'

w "runbooks/Runbook — deploy.md" '# Runbook — deploy

1. Tag, wait for CI.
2. Migrate: `make migrate` — additive only, see [[ADR-002 Postgres over DynamoDB]].
3. Canary 5%, watch 429s from [[Rate limiting]].
4. Full rollout.

If it goes wrong: [[Runbook — rollback]].'

w "runbooks/Runbook — rollback.md" '# Runbook — rollback

Redeploy the previous tag. Migrations are additive, so nothing to undo.
Tell the channel, then write it up in [[Postmortem 2026-05-12]].'

w "runbooks/Runbook — incident.md" '# Runbook — incident

Page, declare, timebox. First question is always whether [[Rate limiting]] is
shedding load or causing it.

Afterwards: [[Postmortem 2026-05-12]], [[Runbook — rollback]].'

w "runbooks/Runbook — rotate secrets.md" '# Runbook — rotate secrets

Quarterly. Two-phase: add the new key, deploy, remove the old.
Do not do this during an incident — see [[Runbook — incident]].'

w "notes/Postmortem 2026-05-12.md" '# Postmortem 2026-05-12

A retry storm from one tenant filled the [[Rate limiting]] bucket for everyone.
Root cause was a client that ignored `Retry-After` and had no
[[Idempotency]] key, so every retry did work.

Actions: per-tenant buckets, and [[ADR-011 Retire the v1 API]].'

w "notes/Why Redis and not Postgres for buckets.md" '# Why Redis and not Postgres for buckets

Counters at the edge need single-digit millisecond reads. Postgres holds the
ledger; see [[ADR-002 Postgres over DynamoDB]] for that decision.

This is the exception, and [[Architecture]] says so.'

w "notes/On-call handover.md" '# On-call handover

Read [[Runbook — incident]] and the last [[Postmortem 2026-05-12]].
Know where [[Rate limiting]] thresholds live.'

w "notes/สรุปการประชุมทีม.md" '# สรุปการประชุมทีม

ตกลงกันว่าจะทำ per-tenant bucket ก่อนสิ้นไตรมาส เพราะเหตุการณ์ใน
[[Postmortem 2026-05-12]] เกิดจาก tenant เดียวกินโควตาทั้งระบบ

เรื่องที่ยังไม่สรุป: จะเลิก v1 เมื่อไหร่ ดู [[ADR-011 Retire the v1 API]]'

w "notes/ตลาดหลักทรัพย์ กับ rate limit ของ vendor.md" '# ตลาดหลักทรัพย์ กับ rate limit ของ vendor

ข้อมูลราคาจากตลาดหลักทรัพย์แห่งประเทศไทยมี rate limit ฝั่ง vendor ที่ 60 ครั้ง
ต่อนาที ต้องกันไว้ที่ gateway ของเราเองด้วย ไม่ใช่พึ่งของเขา

ดู [[Rate limiting]] และ [[Architecture]]'

echo "created $(find "$V" -name '*.md' | wc -l) notes in $V"
