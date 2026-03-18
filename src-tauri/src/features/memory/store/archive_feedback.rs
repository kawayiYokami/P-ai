const MEMORY_DECAY_TIER0_THRESHOLD: f64 = 3.0;
const MEMORY_DECAY_TIER1_THRESHOLD: f64 = 10.0;
const MEMORY_DECAY_CONSOLIDATE_SPEED: f64 = 2.5;
const MEMORY_DECAY_USEFUL_BOOST: i64 = 1;
const MEMORY_DECAY_CYCLE_TIER0_DAYS: i64 = 3;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct MemoryArchiveFeedbackReport {
    recalled_count: usize,
    useful_requested_count: usize,
    useful_accepted_count: usize,
    useful_rejected_count: usize,
    boosted_count: usize,
    penalized_count: usize,
    natural_decay_count: usize,
}

fn memory_decay_tier_of(useful_score: f64) -> i32 {
    if useful_score >= MEMORY_DECAY_TIER1_THRESHOLD {
        2
    } else if useful_score >= MEMORY_DECAY_TIER0_THRESHOLD {
        1
    } else {
        0
    }
}

fn memory_decay_parse_time_or_epoch(raw: Option<&str>) -> OffsetDateTime {
    let text = raw.unwrap_or("").trim();
    if text.is_empty() {
        return OffsetDateTime::UNIX_EPOCH;
    }
    OffsetDateTime::parse(text, &Rfc3339).unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

// ========== apply_useful_boost ==========
fn apply_useful_boost(
    tx: &rusqlite::Transaction,
    useful_accepted: &[String],
    now: &str,
) -> Result<usize, String> {
    let mut boosted_count = 0usize;
    for memory_id in useful_accepted {
        let changed = tx
            .execute(
                "UPDATE memory_record
                 SET strength=strength + ?1,
                     useful_count=useful_count + 1,
                     useful_score=useful_score + ?2,
                     last_recalled_at=?3,
                     updated_at=?3
                 WHERE id=?4",
                params![
                    MEMORY_DECAY_USEFUL_BOOST,
                    MEMORY_DECAY_CONSOLIDATE_SPEED,
                    now,
                    memory_id
                ],
            )
            .map_err(|err| format!("Boost useful memory failed: {err}"))?;
        if changed > 0 {
            boosted_count += 1;
        }
    }
    Ok(boosted_count)
}

// ========== apply_useless_penalty ==========
fn apply_useless_penalty(
    tx: &rusqlite::Transaction,
    recalled_existing: &[String],
    useful_accepted_set: &HashSet<String>,
    now: &str,
) -> Result<usize, String> {
    let mut penalized_count = 0usize;
    for memory_id in recalled_existing {
        if useful_accepted_set.contains(memory_id) {
            continue;
        }
        let useful_score = tx
            .query_row(
                "SELECT useful_score FROM memory_record WHERE id=?1",
                params![memory_id],
                |row| row.get::<_, f64>(0),
            )
            .optional()
            .map_err(|err| format!("Load useful_score for penalty failed: {err}"))?
            .unwrap_or(0.0);
        if memory_decay_tier_of(useful_score) != 1 {
            continue;
        }
        let changed = tx
            .execute(
                "UPDATE memory_record
                 SET strength=MAX(0, strength - 1),
                     updated_at=?1
                 WHERE id=?2",
                params![now, memory_id],
            )
            .map_err(|err| format!("Apply T1 useless penalty failed: {err}"))?;
        if changed > 0 {
            penalized_count += 1;
        }
    }
    Ok(penalized_count)
}

// ========== apply_natural_decay ==========
fn apply_natural_decay(
    tx: &rusqlite::Transaction,
    now: &str,
    now_dt: OffsetDateTime,
) -> Result<usize, String> {
    let mut natural_decay_count = 0usize;
    let cycle_seconds = (MEMORY_DECAY_CYCLE_TIER0_DAYS.max(1) as i128) * 86_400i128;
    let mut t0_stmt = tx
        .prepare(
            "SELECT id, strength, created_at, last_recalled_at, last_decay_at
             FROM memory_record
             WHERE useful_score < ?1 AND strength > 0",
        )
        .map_err(|err| format!("Prepare T0 natural decay query failed: {err}"))?;
    let t0_rows = t0_stmt
        .query_map(params![MEMORY_DECAY_TIER0_THRESHOLD], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })
        .map_err(|err| format!("Query T0 natural decay rows failed: {err}"))?;

    for row in t0_rows {
        let (memory_id, strength, created_at, last_recalled_at, last_decay_at) =
            row.map_err(|err| format!("Read T0 natural decay row failed: {err}"))?;
        let created = memory_decay_parse_time_or_epoch(Some(created_at.as_str()));
        let recalled = memory_decay_parse_time_or_epoch(last_recalled_at.as_deref());
        let decayed = memory_decay_parse_time_or_epoch(last_decay_at.as_deref());
        let ref_time = created.max(recalled).max(decayed);
        let elapsed_seconds = (now_dt.unix_timestamp() as i128) - (ref_time.unix_timestamp() as i128);
        if elapsed_seconds < cycle_seconds {
            continue;
        }
        let steps = (elapsed_seconds / cycle_seconds) as i64;
        if steps <= 0 {
            continue;
        }
        let next_strength = (strength - steps).max(0);
        let next_decay_ts = ref_time.unix_timestamp() as i128 + (steps as i128 * cycle_seconds);
        let next_decay_dt = OffsetDateTime::from_unix_timestamp(next_decay_ts as i64)
            .unwrap_or(now_dt);
        let next_decay_text = next_decay_dt
            .format(&Rfc3339)
            .unwrap_or_else(|_| now.to_string());
        let changed = tx
            .execute(
                "UPDATE memory_record
                 SET strength=?1,
                     last_decay_at=?2,
                     updated_at=?3
                 WHERE id=?4",
                params![next_strength, next_decay_text, now, memory_id],
            )
            .map_err(|err| format!("Apply T0 natural decay failed: {err}"))?;
        if changed > 0 {
            natural_decay_count += 1;
        }
    }
    Ok(natural_decay_count)
}

// ========== main flow ==========
fn memory_store_apply_archive_feedback(
    data_path: &PathBuf,
    recalled_ids: &[String],
    useful_ids: &[String],
) -> Result<MemoryArchiveFeedbackReport, String> {
    let started_at = std::time::Instant::now();
    let mut conn = memory_store_open(data_path)?;
    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|err| format!("Begin archive feedback transaction failed: {err}"))?;

    let normalize_ids = |items: &[String]| -> Vec<String> {
        let mut seen = HashSet::<String>::new();
        let mut out = Vec::<String>::new();
        for raw in items {
            let id = raw.trim();
            if id.is_empty() {
                continue;
            }
            if seen.insert(id.to_string()) {
                out.push(id.to_string());
            }
        }
        out
    };

    let recalled = normalize_ids(recalled_ids);
    let useful = normalize_ids(useful_ids);
    let useful_requested_count = useful.len();
    let now = now_iso();

    let mut recalled_existing = Vec::<String>::new();
    for memory_id in &recalled {
        let exists = tx
            .query_row(
                "SELECT 1 FROM memory_record WHERE id=?1 LIMIT 1",
                params![memory_id],
                |_| Ok(1i64),
            )
            .optional()
            .map_err(|err| format!("Check recalled memory existence failed: {err}"))?
            .is_some();
        if exists {
            recalled_existing.push(memory_id.clone());
        }
    }

    let recalled_set = recalled_existing
        .iter()
        .cloned()
        .collect::<HashSet<String>>();
    let useful_accepted = useful
        .iter()
        .filter(|id| recalled_set.contains(id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let useful_accepted_set = useful_accepted
        .iter()
        .cloned()
        .collect::<HashSet<String>>();
    let useful_rejected_count = useful_requested_count.saturating_sub(useful_accepted.len());
    let boosted_count = apply_useful_boost(&tx, &useful_accepted, &now)?;
    let penalized_count = apply_useless_penalty(&tx, &recalled_existing, &useful_accepted_set, &now)?;
    let natural_decay_count = apply_natural_decay(&tx, &now, OffsetDateTime::now_utc())?;

    tx.commit()
        .map_err(|err| format!("Commit archive feedback transaction failed: {err}"))?;
    runtime_log_info(format!(
        "[记忆存储] [简单记忆回灌] 完成，任务=archive_feedback，recalled_count={}，useful_requested_count={}，useful_accepted_count={}，useful_rejected_count={}，boosted_count={}，penalized_count={}，natural_decay_count={}，elapsed_ms={}",
        recalled_existing.len(),
        useful_requested_count,
        useful_accepted.len(),
        useful_rejected_count,
        boosted_count,
        penalized_count,
        natural_decay_count,
        started_at.elapsed().as_millis()
    ));

    Ok(MemoryArchiveFeedbackReport {
        recalled_count: recalled_existing.len(),
        useful_requested_count,
        useful_accepted_count: useful_accepted.len(),
        useful_rejected_count,
        boosted_count,
        penalized_count,
        natural_decay_count,
    })
}

