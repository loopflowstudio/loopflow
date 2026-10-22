-- Pools name machines you own; on-demands name what you rent and the most it
-- may cost. They share one namespace. A rental is one paid window on a
-- provider, kept separate from the Machine it becomes once reachable.
CREATE TABLE pools (
  name TEXT PRIMARY KEY NOT NULL CHECK(length(trim(name))>0),
  off INTEGER NOT NULL DEFAULT 0 CHECK(off IN (0,1)),
  created_at INTEGER NOT NULL
);

CREATE TABLE ondemands (
  name TEXT PRIMARY KEY NOT NULL CHECK(length(trim(name))>0),
  provider TEXT NOT NULL,
  template TEXT NOT NULL CHECK(json_valid(template)),
  max_machines INTEGER NOT NULL CHECK(max_machines>=0),
  max_usd_per_hour TEXT NOT NULL,
  hours REAL NOT NULL CHECK(hours>0),
  created_at INTEGER NOT NULL
);

-- An owned machine's pool, the cost committed when it was added, and when it
-- stops taking new work. A rented machine leaves all three NULL.
ALTER TABLE machines ADD COLUMN pool TEXT REFERENCES pools(name);
ALTER TABLE machines ADD COLUMN cost_usd TEXT;
ALTER TABLE machines ADD COLUMN expires_at INTEGER;

CREATE TABLE rentals (
  id TEXT PRIMARY KEY NOT NULL CHECK(id GLOB 'rent_*'),
  -- Kept after the on-demand is removed: history outlives configuration.
  ondemand TEXT NOT NULL,
  provider TEXT NOT NULL,
  -- Sent with the create before any reply; finds an unconfirmed create.
  provider_name TEXT NOT NULL UNIQUE,
  provider_id TEXT UNIQUE,
  machine_id TEXT UNIQUE REFERENCES machines(id),
  by_hand INTEGER NOT NULL CHECK(by_hand IN (0,1)),
  hardware TEXT NOT NULL CHECK(json_valid(hardware)),
  usd_per_hour TEXT NOT NULL,
  cap_usd TEXT NOT NULL,
  deadline INTEGER NOT NULL,
  state TEXT NOT NULL CHECK(state IN ('requested','unconfirmed','booting','running','ending','ended','lost')),
  created_at INTEGER NOT NULL,
  ended_at INTEGER,
  cost_usd TEXT,
  -- When the worker's records were last copied here; give-back copies before it deletes.
  copied_at INTEGER
);
CREATE INDEX idx_rentals_ondemand_live ON rentals(ondemand) WHERE state NOT IN ('ended','lost');

-- Session, Process and Graph rows copied from another Machine, as it wrote
-- them. They live apart from this Machine's own rows so no reconciler,
-- attachment or Task judgment here ever acts on a copy; after the origin is
-- deleted they are the final record. observed_at is when they were copied.
CREATE TABLE machine_records (
  origin_machine_id TEXT NOT NULL REFERENCES machines(id),
  kind TEXT NOT NULL CHECK(kind IN ('process','session','session_event','graph_process','graph_process_step')),
  key TEXT NOT NULL,
  row TEXT NOT NULL CHECK(json_valid(row)),
  observed_at INTEGER NOT NULL,
  PRIMARY KEY(origin_machine_id, kind, key)
);
