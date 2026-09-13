pub mod sync;
pub mod updates;

use anyhow::{bail, ensure, Context, Result};
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveDate, NaiveDateTime, TimeZone, Utc};
use chrono_tz::Tz;
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use rand::{rngs::OsRng, Rng};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use uuid::Uuid;

pub const WEEK: i64 = 7 * 24 * 60 * 60 * 1000;
pub fn now() -> i64 {
    Utc::now().timestamp_millis()
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn bytes<const N: usize>(s: &str) -> Result<[u8; N]> {
    hex::decode(s)?
        .try_into()
        .map_err(|_| anyhow::anyhow!("Invalid key length"))
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub public_key: String,
    pub noise_key: String,
    pub order: u64,
    pub sponsor: String,
    pub signature: String,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub timezone: String,
    pub root: String,
    pub members: Vec<Member>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Identity {
    pub id: String,
    pub name: String,
    seed: String,
    pub noise_private: String,
    pub noise_public: String,
}
impl Identity {
    fn new(name: String) -> Result<Self> {
        let key = SigningKey::generate(&mut OsRng);
        let noise = snow::Builder::new(sync::NOISE.parse()?).generate_keypair()?;
        Ok(Self {
            id: hex::encode(key.verifying_key().to_bytes()),
            name,
            seed: hex::encode(key.to_bytes()),
            noise_private: hex::encode(noise.private),
            noise_public: hex::encode(noise.public),
        })
    }
    fn key(&self) -> Result<SigningKey> {
        Ok(SigningKey::from_bytes(&bytes(&self.seed)?))
    }
    pub fn member(&self) -> Member {
        Member {
            id: self.id.clone(),
            name: self.name.clone(),
            public_key: self.id.clone(),
            noise_key: self.noise_public.clone(),
            order: 0,
            sponsor: String::new(),
            signature: String::new(),
        }
    }
}
fn member_message(group: &str, m: &Member) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec(&(
        group,
        &m.id,
        &m.name,
        &m.public_key,
        &m.noise_key,
        m.order,
        &m.sponsor,
    ))?)
}
impl Group {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.name.len() <= 100 && self.members.len() <= 64,
            "Invalid group"
        );
        let _: Tz = self.timezone.parse().context("Invalid timezone")?;
        let mut verified = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for m in &self.members {
            ensure!(
                ids.insert(&m.id) && m.id == m.public_key,
                "Invalid member identity"
            );
        }
        for _ in 0..self.members.len() {
            for m in &self.members {
                if verified.contains(&m.id) {
                    continue;
                }
                let root = m.id == self.root && m.sponsor == m.id && m.order == 0;
                if !root && !verified.contains(&m.sponsor) {
                    continue;
                }
                let sponsor = self
                    .members
                    .iter()
                    .find(|s| s.id == m.sponsor)
                    .context("Missing sponsor")?;
                ensure!(root || m.order > sponsor.order, "Invalid joining order");
                VerifyingKey::from_bytes(&bytes(&sponsor.public_key)?)?.verify(
                    &member_message(&self.id, m)?,
                    &Signature::from_bytes(&bytes(&m.signature)?),
                )?;
                verified.insert(&m.id);
            }
        }
        ensure!(
            verified.len() == self.members.len() && verified.contains(&self.root),
            "Untrusted group membership"
        );
        Ok(())
    }
    fn priority(&self, author: &str) -> (u64, String) {
        self.members
            .iter()
            .find(|m| m.id == author)
            .map(|m| (m.order, m.id.clone()))
            .unwrap_or((0, author.to_string()))
    }
}

#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Category {
    pub id: String,
    pub name: String,
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Entry {
    pub id: String,
    pub category: String,
    pub start: i64,
    pub end: i64,
    pub created: i64,
}
#[derive(Clone, Serialize, Deserialize, Debug, PartialEq)]
pub struct Timer {
    pub id: String,
    pub category: String,
    pub start: i64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Change {
    Category(Category),
    CategoryColor { id: String, color: String },
    DevicePriority { devices: Vec<String> },
    Entry(Entry),
    Delete { id: String, created: i64 },
    Timer { id: String, timer: Option<Timer> },
}
impl Change {
    fn target(&self) -> String {
        match self {
            Self::Category(v) => format!("c:{}", v.id),
            Self::CategoryColor { id, .. } => format!("color:{id}"),
            Self::DevicePriority { .. } => "device-priority".into(),
            Self::Entry(v) => format!("e:{}", v.id),
            Self::Delete { id, .. } => format!("e:{id}"),
            Self::Timer { id, .. } => format!("t:{id}"),
        }
    }
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Operation {
    pub id: String,
    pub author: String,
    pub clock: BTreeMap<String, u64>,
    pub at: i64,
    pub change: Change,
    pub signature: String,
}
impl Operation {
    fn message(&self) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&(
            &self.id,
            &self.author,
            &self.clock,
            self.at,
            &self.change,
        ))?)
    }
    fn validate(&self, group: &Group) -> Result<()> {
        let m = group
            .members
            .iter()
            .find(|m| m.id == self.author)
            .context("Unknown operation author")?;
        VerifyingKey::from_bytes(&bytes(&m.public_key)?)?.verify(
            &self.message()?,
            &Signature::from_bytes(&bytes(&self.signature)?),
        )?;
        let seq = self
            .clock
            .get(&self.author)
            .context("Missing operation clock")?;
        ensure!(
            *seq > 0 && self.id == format!("{}:{seq}", self.author),
            "Invalid operation id"
        );
        ensure!(self.at >= 0, "Invalid operation time");
        match &self.change {
            Change::Category(c) => ensure!(
                !c.name.trim().is_empty() && c.name.len() <= 80,
                "Invalid category"
            ),
            Change::CategoryColor { color, .. } => ensure!(
                color.len() == 7
                    && color.starts_with('#')
                    && color[1..].bytes().all(|b| b.is_ascii_hexdigit()),
                "Use a color in #RRGGBB format"
            ),
            Change::DevicePriority { devices } => ensure!(
                !devices.is_empty()
                    && devices.len() <= 64
                    && devices.iter().collect::<BTreeSet<_>>().len() == devices.len()
                    && devices
                        .iter()
                        .all(|id| group.members.iter().any(|m| &m.id == id)),
                "Invalid device priority"
            ),
            Change::Entry(e) => ensure!(
                e.start >= 0
                    && e.start < e.end
                    && e.end <= self.at
                    && e.created <= self.at
                    && self.at < e.created + WEEK,
                "Invalid entry or expired editing window"
            ),
            Change::Delete { created, .. } => ensure!(
                *created <= self.at && self.at < created + WEEK,
                "Expired deletion"
            ),
            Change::Timer { timer: Some(t), .. } => {
                ensure!(t.start >= 0 && t.start <= self.at, "Invalid timer")
            }
            _ => {}
        }
        Ok(())
    }
}
fn after(a: &Operation, b: &Operation) -> bool {
    b.clock
        .iter()
        .all(|(k, v)| a.clock.get(k).unwrap_or(&0) >= v)
        && a.clock != b.clock
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Replica {
    pub group: Group,
    pub operations: Vec<Operation>,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Config {
    pub release_url: String,
    pub release_key: String,
    pub last_update_check: i64,
}
#[derive(Clone, Serialize, Deserialize)]
struct Meta {
    identity: Identity,
    group: Group,
    config: Config,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Snapshot {
    pub device_priority: Vec<String>,
    pub category_colors: BTreeMap<String, String>,
    pub device_id: String,
    pub group: Group,
    pub categories: Vec<Category>,
    pub entries: Vec<Entry>,
    pub timer: Option<Timer>,
    pub excluded_count: usize,
    pub config: Config,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct CategoryTotal {
    pub id: String,
    pub name: String,
    pub duration: i64,
    pub share: f64,
}
#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Report {
    pub start: i64,
    pub end: i64,
    pub recorded: i64,
    pub gaps: i64,
    pub categories: Vec<CategoryTotal>,
    pub provisional: bool,
}
pub struct Store {
    conn: Connection,
    meta: Meta,
    operations: Vec<Operation>,
}
impl Store {
    pub fn open(path: impl AsRef<Path>, name: String, timezone: &str) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON; CREATE TABLE IF NOT EXISTS meta (id INTEGER PRIMARY KEY CHECK(id=1), value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS operations (id TEXT PRIMARY KEY, value TEXT NOT NULL);")?;
        let saved: Option<String> = conn
            .query_row("SELECT value FROM meta WHERE id=1", [], |r| r.get(0))
            .optional()?;
        let meta = if let Some(s) = saved {
            serde_json::from_str(&s)?
        } else {
            let _: Tz = timezone.parse()?;
            let identity = Identity::new(name)?;
            let mut group = Group {
                id: id(),
                name: "My devices".into(),
                timezone: timezone.into(),
                root: identity.id.clone(),
                members: vec![],
            };
            let mut member = identity.member();
            member.sponsor = identity.id.clone();
            member.signature = hex::encode(
                identity
                    .key()?
                    .sign(&member_message(&group.id, &member)?)
                    .to_bytes(),
            );
            group.members.push(member);
            Meta {
                identity,
                group,
                config: Config {
                    release_url: String::new(),
                    release_key: String::new(),
                    last_update_check: 0,
                },
            }
        };
        let operations = {
            let mut stmt = conn.prepare("SELECT value FROM operations ORDER BY id")?;
            let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
            let mut ops = vec![];
            for r in rows {
                ops.push(serde_json::from_str(&r?)?);
            }
            ops
        };
        let mut s = Self {
            conn,
            meta,
            operations,
        };
        s.persist()?;
        Ok(s)
    }
    fn persist(&mut self) -> Result<()> {
        let tx = self.conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO meta VALUES(1,?1)",
            [serde_json::to_string(&self.meta)?],
        )?;
        for op in &self.operations {
            tx.execute(
                "INSERT OR IGNORE INTO operations VALUES(?1,?2)",
                params![op.id, serde_json::to_string(op)?],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn identity(&self) -> Identity {
        self.meta.identity.clone()
    }
    pub fn replica(&self) -> Replica {
        Replica {
            group: self.meta.group.clone(),
            operations: self.operations.clone(),
        }
    }
    fn append(&mut self, changes: Vec<Change>, at: i64) -> Result<()> {
        let before = self.operations.len();
        for change in changes {
            let mut clock = BTreeMap::new();
            for op in &self.operations {
                for (k, v) in &op.clock {
                    let c = clock.entry(k.clone()).or_insert(0);
                    *c = (*c).max(*v);
                }
            }
            let author = self.meta.identity.id.clone();
            let seq = clock.entry(author.clone()).or_insert(0);
            *seq += 1;
            let mut op = Operation {
                id: format!("{author}:{seq}"),
                author,
                clock,
                at,
                change,
                signature: String::new(),
            };
            op.signature = hex::encode(self.meta.identity.key()?.sign(&op.message()?).to_bytes());
            if let Err(e) = op.validate(&self.meta.group) {
                self.operations.truncate(before);
                return Err(e);
            }
            self.operations.push(op);
        }
        if let Err(e) = self.persist() {
            self.operations.truncate(before);
            return Err(e);
        }
        Ok(())
    }
    pub fn admit(&mut self, mut member: Member) -> Result<Group> {
        ensure!(self.meta.group.members.len() < 64, "Group is full");
        if let Some(existing) = self.meta.group.members.iter().find(|m| m.id == member.id) {
            ensure!(
                existing.noise_key == member.noise_key,
                "Device identity changed"
            );
            return Ok(self.meta.group.clone());
        }
        member.order = self
            .meta
            .group
            .members
            .iter()
            .map(|m| m.order)
            .max()
            .unwrap_or(0)
            + 1;
        member.sponsor = self.meta.identity.id.clone();
        member.signature = hex::encode(
            self.meta
                .identity
                .key()?
                .sign(&member_message(&self.meta.group.id, &member)?)
                .to_bytes(),
        );
        let mut group = self.meta.group.clone();
        group.members.push(member);
        group.validate()?;
        self.meta.group = group;
        self.persist()?;
        Ok(self.meta.group.clone())
    }
    pub fn join(&mut self, replica: Replica) -> Result<()> {
        ensure!(
            self.meta.group.members.len() == 1,
            "This device already belongs to a shared group"
        );
        replica.group.validate()?;
        let me = replica
            .group
            .members
            .iter()
            .find(|m| m.id == self.meta.identity.id)
            .context("Device was not admitted")?;
        ensure!(
            me.noise_key == self.meta.identity.noise_public,
            "Identity mismatch"
        );
        let old = self.meta.group.clone();
        self.meta.group = replica.group.clone();
        if let Err(e) = self.merge(replica) {
            self.meta.group = old;
            return Err(e);
        }
        Ok(())
    }
    pub fn merge(&mut self, replica: Replica) -> Result<()> {
        ensure!(
            replica.group.id == self.meta.group.id
                && replica.group.root == self.meta.group.root
                && replica.group.timezone == self.meta.group.timezone,
            "Different group"
        );
        replica.group.validate()?;
        let mut group = self.meta.group.clone();
        for m in replica.group.members {
            if let Some(existing) = group.members.iter_mut().find(|x| x.id == m.id) {
                // Concurrent admission of the same identity also converges deterministically.
                if (m.order, &m.sponsor, &m.signature)
                    > (existing.order, &existing.sponsor, &existing.signature)
                {
                    *existing = m;
                }
            } else {
                group.members.push(m);
            }
        }
        group.members.sort_by(|a, b| a.id.cmp(&b.id));
        group.validate()?;
        let mut combined: BTreeMap<String, Operation> = self
            .operations
            .iter()
            .map(|o| (o.id.clone(), o.clone()))
            .collect();
        for op in replica.operations {
            op.validate(&group)?;
            if let Some(old) = combined.get(&op.id) {
                ensure!(
                    old.signature == op.signature,
                    "Conflicting operation identity"
                );
            } else {
                combined.insert(op.id.clone(), op);
            }
        }
        let mut created = BTreeMap::new();
        for op in combined.values() {
            match &op.change {
                Change::Entry(e) => {
                    if let Some(t) = created.insert(e.id.clone(), e.created) {
                        ensure!(t == e.created, "Creation time is immutable");
                    }
                }
                Change::Delete { id, created: t } => {
                    if let Some(old) = created.insert(id.clone(), *t) {
                        ensure!(old == *t, "Creation time is immutable");
                    }
                }
                _ => {}
            }
        }
        let old_ops = std::mem::replace(&mut self.operations, combined.into_values().collect());
        let old_group = std::mem::replace(&mut self.meta.group, group);
        if let Err(e) = self.persist() {
            self.operations = old_ops;
            self.meta.group = old_group;
            return Err(e);
        }
        Ok(())
    }
    // Resolve ordering changes using immutable admission priority to avoid a
    // circular dependency between the order and the operation choosing it.
    fn device_priority(&self) -> Vec<String> {
        let ops: Vec<_> = self
            .operations
            .iter()
            .filter(|o| matches!(o.change, Change::DevicePriority { .. }))
            .collect();
        let winner = ops
            .iter()
            .filter(|a| !ops.iter().any(|b| after(b, a)))
            .max_by_key(|o| (self.meta.group.priority(&o.author), o.id.clone()));
        let chosen = match winner.map(|o| &o.change) {
            Some(Change::DevicePriority { devices }) => devices.clone(),
            _ => vec![],
        };
        let mut members = self.meta.group.members.clone();
        members.sort_by_key(|m| std::cmp::Reverse((m.order, m.id.clone())));
        let mut result: Vec<_> = members
            .iter()
            .filter(|m| !chosen.contains(&m.id))
            .map(|m| m.id.clone())
            .collect();
        result.extend(
            chosen
                .into_iter()
                .filter(|id| members.iter().any(|m| &m.id == id)),
        );
        result
    }
    pub fn set_device_priority(&mut self, devices: Vec<String>, at: i64) -> Result<()> {
        ensure!(
            devices.len() == self.meta.group.members.len(),
            "Include every device exactly once"
        );
        self.append(vec![Change::DevicePriority { devices }], at)
    }
    pub fn set_category_color(&mut self, category: &str, color: &str, at: i64) -> Result<()> {
        self.has_category(category)?;
        self.append(
            vec![Change::CategoryColor {
                id: category.into(),
                color: color.to_ascii_lowercase(),
            }],
            at,
        )
    }
    pub fn snapshot(&self) -> Snapshot {
        let device_priority = self.device_priority();
        let priority = |author: &str| {
            device_priority.len()
                - device_priority
                    .iter()
                    .position(|id| id == author)
                    .unwrap_or(device_priority.len())
        };
        let mut targets: BTreeMap<String, Vec<&Operation>> = BTreeMap::new();
        for op in &self.operations {
            targets.entry(op.change.target()).or_default().push(op);
        }
        let winners: Vec<&Operation> = targets
            .values()
            .filter_map(|ops| {
                ops.iter()
                    .filter(|a| !ops.iter().any(|b| after(b, a)))
                    .max_by_key(|o| (priority(&o.author), o.id.clone()))
                    .copied()
            })
            .collect();
        let mut categories: Vec<Category> = winners
            .iter()
            .filter_map(|o| {
                if let Change::Category(c) = &o.change {
                    Some(c.clone())
                } else {
                    None
                }
            })
            .collect();
        categories.sort_by_key(|c| (c.name.to_lowercase(), c.id.clone()));
        let mut aliases = BTreeMap::new();
        let mut canonical: BTreeMap<String, String> = BTreeMap::new();
        categories.retain(|c| {
            let k = c.name.trim().to_lowercase();
            if let Some(existing) = canonical.get(&k) {
                aliases.insert(c.id.clone(), existing.clone());
                false
            } else {
                canonical.insert(k, c.id.clone());
                true
            }
        });
        let mut category_colors = BTreeMap::new();
        for o in &winners {
            if let Change::CategoryColor { id, color } = &o.change {
                // Duplicate category names use the canonical category's color.
                if categories.iter().any(|c| &c.id == id) {
                    category_colors.insert(id.clone(), color.clone());
                }
            }
        }
        let mut candidates: Vec<&Operation> = winners
            .into_iter()
            .filter(|o| {
                matches!(
                    o.change,
                    Change::Entry(_) | Change::Timer { timer: Some(_), .. }
                )
            })
            .collect();
        candidates.sort_by_key(|o| std::cmp::Reverse((priority(&o.author), o.id.clone())));
        let mut spans: Vec<(i64, i64)> = vec![];
        let mut entries = vec![];
        let mut timer = None;
        let mut excluded_count = 0;
        for op in candidates {
            let (start, end) = match &op.change {
                Change::Entry(e) => (e.start, e.end),
                Change::Timer { timer: Some(t), .. } => (t.start, i64::MAX),
                _ => unreachable!(),
            };
            if spans.iter().any(|(s, e)| start < *e && end > *s)
                || (matches!(op.change, Change::Timer { .. }) && timer.is_some())
            {
                excluded_count += 1;
                continue;
            }
            spans.push((start, end));
            match &op.change {
                Change::Entry(e) => {
                    let mut e = e.clone();
                    if let Some(c) = aliases.get(&e.category) {
                        e.category = c.clone();
                    }
                    entries.push(e);
                }
                Change::Timer { timer: Some(t), .. } => {
                    let mut t = t.clone();
                    if let Some(c) = aliases.get(&t.category) {
                        t.category = c.clone();
                    }
                    timer = Some(t);
                }
                _ => {}
            }
        }
        entries.sort_by_key(|e| e.start);
        Snapshot {
            device_priority,
            category_colors,
            device_id: self.meta.identity.id.clone(),
            group: self.meta.group.clone(),
            categories,
            entries,
            timer,
            excluded_count,
            config: self.meta.config.clone(),
        }
    }
    pub fn add_category(&mut self, name: &str, at: i64) -> Result<Category> {
        let name = name.trim();
        ensure!(
            !name.is_empty() && name.len() <= 80,
            "Use a category name of 1–80 characters"
        );
        if let Some(c) = self
            .snapshot()
            .categories
            .into_iter()
            .find(|c| c.name.to_lowercase() == name.to_lowercase())
        {
            return Ok(c);
        }
        let c = Category {
            id: id(),
            name: name.into(),
        };
        let palette = [
            "#b77340", "#90b341", "#40aab5", "#6b83c7", "#9270bc", "#bb6e92", "#bb645c", "#b79843",
            "#579a79", "#728c9a", "#a47b60", "#7079b5",
        ];
        let color = palette[OsRng.gen_range(0..palette.len())].to_string();
        self.append(
            vec![
                Change::Category(c.clone()),
                Change::CategoryColor {
                    id: c.id.clone(),
                    color,
                },
            ],
            at,
        )?;
        Ok(c)
    }
    fn has_category(&self, category: &str) -> Result<()> {
        ensure!(
            self.snapshot().categories.iter().any(|c| c.id == category),
            "Choose an existing category"
        );
        Ok(())
    }
    /// Preserve the precise timer timestamp when its minute-resolution form value
    /// was left unchanged. Editing only a category must not round a short timer to zero.
    pub fn save_entry_local(
        &mut self,
        entry_id: Option<String>,
        category: String,
        start: &str,
        end: &str,
        at: i64,
    ) -> Result<Entry> {
        let existing = self
            .snapshot()
            .entries
            .into_iter()
            .find(|e| Some(&e.id) == entry_id.as_ref());
        let resolve = |value: &str, old: Option<i64>| -> Result<i64> {
            if let Some(old) = old {
                if format_local(old, &self.meta.group.timezone)? == value {
                    return Ok(old);
                }
            }
            self.parse_local(value)
        };
        let start = resolve(start, existing.as_ref().map(|e| e.start))?;
        let end = resolve(end, existing.as_ref().map(|e| e.end))?;
        self.save_entry(entry_id, category, start, end, at)
    }
    pub fn save_entry(
        &mut self,
        entry_id: Option<String>,
        category: String,
        start: i64,
        end: i64,
        at: i64,
    ) -> Result<Entry> {
        self.has_category(&category)?;
        ensure!(start >= 0 && start < end, "End must be after start");
        ensure!(end <= at, "Completed entries cannot end in the future");
        let snap = self.snapshot();
        let existing = if let Some(ref eid) = entry_id {
            Some(
                snap.entries
                    .iter()
                    .find(|e| &e.id == eid)
                    .context("Entry not found")?,
            )
        } else {
            None
        };
        if let Some(e) = existing {
            ensure!(
                at < e.created + WEEK,
                "This entry’s seven-day editing window has ended"
            );
        }
        ensure!(
            !snap
                .entries
                .iter()
                .any(|e| Some(&e.id) != entry_id.as_ref() && start < e.end && end > e.start),
            "This time overlaps an existing entry"
        );
        ensure!(
            !snap.timer.as_ref().is_some_and(|t| end > t.start),
            "This time overlaps the running timer"
        );
        let e = Entry {
            id: entry_id.unwrap_or_else(id),
            category,
            start,
            end,
            created: existing.map(|e| e.created).unwrap_or(at),
        };
        self.append(vec![Change::Entry(e.clone())], at)?;
        Ok(e)
    }
    pub fn delete_entry(&mut self, eid: &str, at: i64) -> Result<()> {
        let e = self
            .snapshot()
            .entries
            .into_iter()
            .find(|e| e.id == eid)
            .context("Entry not found")?;
        ensure!(at < e.created + WEEK, "This entry is locked");
        self.append(
            vec![Change::Delete {
                id: e.id,
                created: e.created,
            }],
            at,
        )
    }
    pub fn start_timer(&mut self, category: String, at: i64) -> Result<Timer> {
        self.has_category(&category)?;
        ensure!(
            self.snapshot().timer.is_none(),
            "Stop the current timer first"
        );
        let t = Timer {
            id: id(),
            category,
            start: at,
        };
        self.append(
            vec![Change::Timer {
                id: t.id.clone(),
                timer: Some(t.clone()),
            }],
            at,
        )?;
        Ok(t)
    }
    pub fn stop_timer(&mut self, at: i64) -> Result<Entry> {
        let t = self.snapshot().timer.context("No running timer")?;
        ensure!(at > t.start, "The timer has not elapsed yet");
        let e = Entry {
            id: t.id.clone(),
            category: t.category,
            start: t.start,
            end: at,
            created: at,
        };
        self.append(
            vec![
                Change::Timer {
                    id: t.id,
                    timer: None,
                },
                Change::Entry(e.clone()),
            ],
            at,
        )?;
        Ok(e)
    }
    pub fn set_updates(&mut self, url: String, key: String) -> Result<()> {
        if !url.is_empty() {
            updates::validate_endpoint(&url)?;
            bytes::<32>(&key)?;
        }
        self.meta.config.release_url = url;
        self.meta.config.release_key = key;
        self.meta.config.last_update_check = 0;
        self.persist()
    }
    pub fn mark_update_check(&mut self, at: i64) -> Result<()> {
        self.meta.config.last_update_check = at;
        self.persist()
    }
    pub fn parse_local(&self, value: &str) -> Result<i64> {
        let naive = NaiveDateTime::parse_from_str(value, "%Y-%m-%dT%H:%M")?;
        let tz: Tz = self.meta.group.timezone.parse()?;
        match tz.from_local_datetime(&naive) {
            LocalResult::Single(d) => Ok(d.timestamp_millis()),
            LocalResult::Ambiguous(..) => bail!(
                "This local time occurs twice during a clock change; choose an unambiguous time"
            ),
            LocalResult::None => bail!("This local time does not exist because of a clock change"),
        }
    }
    pub fn report(&self, period: &str, date: &str, at: i64) -> Result<Report> {
        let date = NaiveDate::parse_from_str(date, "%Y-%m-%d")?;
        let tz: Tz = self.meta.group.timezone.parse()?;
        let start = match period {
            "day" => date,
            "week" => date - Duration::days(date.weekday().num_days_from_monday() as i64),
            "month" => date.with_day(1).unwrap(),
            _ => bail!("Invalid reporting period"),
        };
        let end = match period {
            "day" => start + Duration::days(1),
            "week" => start + Duration::days(7),
            _ => {
                if start.month() == 12 {
                    NaiveDate::from_ymd_opt(start.year() + 1, 1, 1).unwrap()
                } else {
                    NaiveDate::from_ymd_opt(start.year(), start.month() + 1, 1).unwrap()
                }
            }
        };
        // Find the first valid minute if a timezone skips midnight.
        let midnight = |d: NaiveDate| -> Result<i64> {
            for m in 0..=1440 {
                let n = d.and_hms_opt(0, 0, 0).unwrap() + Duration::minutes(m);
                if let Some(v) = tz.from_local_datetime(&n).earliest() {
                    return Ok(v.timestamp_millis());
                }
            }
            bail!("Invalid date boundary")
        };
        let start = midnight(start)?;
        let end = midnight(end)?.min(at).max(start);
        let snap = self.snapshot();
        let mut totals: BTreeMap<String, i64> = BTreeMap::new();
        let mut add = |cat: &str, s: i64, e: i64| {
            let duration = (e.min(end) - s.max(start)).max(0);
            *totals.entry(cat.into()).or_default() += duration;
        };
        for e in &snap.entries {
            add(&e.category, e.start, e.end);
        }
        let mut provisional = false;
        if let Some(t) = &snap.timer {
            add(&t.category, t.start, at);
            provisional = t.start < end && at > start;
        }
        let recorded = totals.values().sum::<i64>();
        let mut categories: Vec<CategoryTotal> = snap
            .categories
            .iter()
            .map(|c| {
                let duration = *totals.get(&c.id).unwrap_or(&0);
                CategoryTotal {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    duration,
                    share: if recorded > 0 {
                        duration as f64 / recorded as f64
                    } else {
                        0.
                    },
                }
            })
            .filter(|c| c.duration > 0)
            .collect();
        categories.sort_by(|a, b| b.duration.cmp(&a.duration).then(a.name.cmp(&b.name)));
        Ok(Report {
            start,
            end,
            recorded,
            gaps: (end - start - recorded).max(0),
            categories,
            provisional,
        })
    }
}

pub fn format_local(ms: i64, timezone: &str) -> Result<String> {
    let tz: Tz = timezone.parse()?;
    Ok(DateTime::from_timestamp_millis(ms)
        .context("Invalid timestamp")?
        .with_timezone(&tz)
        .format("%Y-%m-%dT%H:%M")
        .to_string())
}
