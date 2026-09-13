export interface Category {
  id: string;
  name: string;
}
export interface Entry {
  id: string;
  category: string;
  start: number;
  end: number;
  created: number;
}
export interface Timer {
  id: string;
  category: string;
  start: number;
}
export interface Member {
  id: string;
  name: string;
  order: number;
  sponsor: string;
}
export interface Group {
  id: string;
  name: string;
  timezone: string;
  root: string;
  members: Member[];
}
export interface Config {
  release_url: string;
  release_key: string;
  last_update_check: number;
}
export interface Snapshot {
  device_id: string;
  group: Group;
  categories: Category[];
  entries: Entry[];
  timer: Timer | null;
  excluded_count: number;
  config: Config;
}
export interface Report {
  start: number;
  end: number;
  recorded: number;
  gaps: number;
  categories: { id: string; name: string; duration: number; share: number }[];
  provisional: boolean;
}
export interface SyncStatus {
  port: number;
  peers: { name: string; group_id: string; address: string }[];
  pending: { id: string; name: string; code: string }[];
  joining_code: string | null;
  last_sync: number | null;
  error: string | null;
  active: boolean;
}
export interface Manifest {
  version: string;
  version_code: number;
  apk_url: string;
  sha256: string;
  signature: string;
}
