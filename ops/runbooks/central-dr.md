# Central disaster recovery runbook (draft)

**Status: draft.** The standby site, the key custodians and the drill dates are not settled yet. Each is marked *to settle* below; [Stand up central disaster recovery](https://github.com/carapai/people-centered-healthcare/issues/138) (#138) closes when they are. The design is ADR 0046 (backups and standby), ADR 0065 (edges re-push after a restore) and [deployment.md § Disaster recovery](../../docs/architecture/deployment.md).

## Targets

| | Target |
|---|---|
| RPO, global records (registries, metadata, identity, permissions) | 1 h: the backup interval |
| RPO, facility records | None lost: edges hold them and re-push what the backup lacks (ADR 0065) |
| RTO | 4 h, from the decision to fail over to central serving again |

Facilities keep working offline throughout. Only central-only functions stop: payer and payment integrations, NIN checks, golden-record matching, claims, analytics, and metadata and price authoring.

## Roles

| Role | Who | Does |
|---|---|---|
| Incident lead | *to settle* | Declares the disaster, decides to fail over, and keeps the record |
| Operator | *to settle* | Runs the commands below |
| Key custodians | Five people, three needed (*to settle*: names, deputies and where each share is kept) | Bring their shares of the backup key |
| Witness | One person other than the operator (*to settle*) | Checks each step against this runbook, and signs the record |

## The backup key

- Central's backups are encrypted under one 256-bit key (`HEALTH_BACKUP_KEY_FILE`). Without it no backup can be restored.
- **Creation (once):**
  1. The operator runs `central-server backup key <path>` on the production host, with the custodians and the witness present.
  2. The key is split into five Shamir shares with a threshold of three. *To settle:* the tool, for example `ssss-split -t 3 -n 5` on an offline machine.
  3. Each custodian takes one share on paper or on encrypted media.
  4. The operator records the key's SHA-256 in the record, never the key itself.
- **The running server** keeps the key file at `HEALTH_BACKUP_KEY_FILE`, owner-only. Moving it into the KMS is later work.
- **The standby host** does not hold the key. Three custodians bring their shares when it is needed.
- **Rotation:** when a custodian leaves, or a share may be exposed:
  1. Create a new key and split it as above.
  2. Point `HEALTH_BACKUP_KEY_FILE` at the new key.
  3. Keep the old key's shares until the last backup made under it has aged out of retention (12 months).

## Where the backups are

| Copy | Where | Kept |
|---|---|---|
| Primary | `HEALTH_BACKUP_DIR` on a volume apart from the database | 35 daily, 12 weekly, 12 monthly |
| Offsite in-country | *to settle*: the second site. Fed by `central-server backup copy <dir>` at least daily, or by replicating `HEALTH_BACKUP_DIR` | The same |

Each backup holds the database export and the attachments, with a manifest of their checksums. `central-server backup verify` checks them all.

## Daily and weekly checks

- **Alerts:** `backup-failed`, `backup-stale`, `backup-test-failed` and `backups-off` are critical (ops/observability). Act on them the same day.
- **Weekly:** the server test-restores the newest backup into `restore_test` by itself. Its outcome is in `status.json` (`central-server backup status`).

## Failing over to the standby

The standby is a VM at the second in-country site with the same central-server release, a SurrealDB server and an empty `HEALTH_FILES_DIR`. *To settle:* the site, the VM and its network path to the facilities.

1. **Declare.** The incident lead declares the disaster and opens the record: the time, what failed, and who is present.
2. **Fence the old primary.** If it might come back, stop central-server there, or cut it off, so that edges talk to one central only.
3. **Bring the key.** Three custodians rebuild the key on the standby, with the witness present, into an owner-only file. Check its SHA-256 against the record.
4. **Pick the backup.** Use the newest that verifies: `central-server backup verify` on the offsite copy. Note its time: global records written after it are lost.
5. **Restore.**

   ```sh
   central-server backup restore <backup-dir> --into main_<yyyymmdd>
   ```

   It restores into a new database and the attachments into `HEALTH_FILES_DIR`, and gives the store a new sync epoch.
6. **Start.** Set `HEALTH_DATABASE=main_<yyyymmdd>` and start central-server. Wait for `/ready`.
7. **Move the name.** Point central's DNS name at the standby. Edges trust central through its certificate, so the standby must serve one for the same name.
8. **Watch the re-sync.**
   - Each edge sees central's new epoch and logs `sync-repush` with how many entries it queued again.
   - It pushes them, re-seeds its pull side, and syncs as before.
   - Central admits at most `HEALTH_SYNC_MAX_CONCURRENT_PUSHES` re-syncs at once, so expect them to queue (`429`) for a while.
   - Done when no `sync-failing` or `sync-reseed` alert is open for any edge.
9. **Reconcile.** Compare each edge's ledger digests and receipt counts with central's (deployment.md, "Post-recovery reconciliation"). Record any difference.
10. **Close.** Record when central served again (the RTO), what was lost (global records after the backup), and anything that went wrong. Rotate the backup key if its shares were handled outside the custodians' control.

## The quarterly drill

- **When:** every quarter (*to settle*: the dates).
- **Where:** into staging, never production.
- **What:** run *Failing over to the standby* against staging, from step 3. Use the newest offsite backup and a staging edge.
- **It passes when:**
  - central serves within 4 h of starting step 3;
  - a staging edge with writes made after the backup logs `sync-repush`, and its records reach central;
  - the reconciliation shows no difference.
- **The record:** the date, who took part, the times of each step, and the result. Keep it with the incident records.

## Not covered yet

- The standby VM, its site and its DNS failover (*to settle*).
- Edges' own backup keys, escrowed at central (ADR 0046 § Keys).
- Break-glass grants, transfers and activation reports received in the lost hour: their queues do not re-send after a restore (ADR 0065).
