export type PropertyType =
  | 'text'
  | 'number'
  | 'currency'
  | 'date'
  | 'select'
  | 'multi_select'
  | 'checkbox'
  | 'relation';

export type PropertyValue =
  | { kind: 'text'; value: string }
  | { kind: 'number'; value: number }
  | { kind: 'date'; value: string }
  | { kind: 'bool'; value: boolean }
  | { kind: 'list'; value: string[] }
  | { kind: 'null'; value?: null };

export interface PropertyPatch {
  key: string;
  value: PropertyValue | null;
}

export interface PropertySchema {
  label: string;
  type: PropertyType;
  unit?: string;
  options?: string[];
}

export interface DocumentMeta {
  id: string;
  rel_path: string;
  stem: string;
  folder: string;
  title: string;
  doc_type: string | null;
  content_hash: string;
  mtime_ms: number;
  size_bytes: number;
  created_at: string;
  updated_at: string;
}

export interface Wikilink {
  raw_target: string;
  anchor: string | null;
  alias: string | null;
  is_embed: boolean;
}

export interface BacklinkEntry {
  source_doc_id: string;
  source_rel_path: string;
  source_title: string;
  raw_target: string;
  anchor: string | null;
  alias: string | null;
}

export interface TaskItem {
  line_no: number;
  completed: boolean;
  body: string;
}

export interface DocumentDetail {
  meta: DocumentMeta;
  raw_md: string;
  body_md: string;
  properties: Record<string, PropertyValue>;
  ordered_keys: string[];
  tags: string[];
  wikilinks: Wikilink[];
  backlinks: BacklinkEntry[];
  tasks: TaskItem[];
}

export interface SaveResult {
  rel_path: string;
  new_hash: string;
  updated_at: string;
}

export type ViewType = 'table' | 'board' | 'calendar';
export type SortDir = 'asc' | 'desc';
export type AggregationOp = 'count' | 'sum' | 'avg' | 'min' | 'max' | 'checked_ratio';

export type FilterOp =
  | 'eq'
  | 'neq'
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte'
  | 'contains'
  | 'starts_with'
  | 'in'
  | 'between'
  | 'is_empty'
  | 'is_not_empty';

export type FilterExpr =
  | { and: FilterExpr[] }
  | { or: FilterExpr[] }
  | { not: FilterExpr }
  | {
      op: FilterOp;
      property: string;
      value?: unknown;
      values?: unknown[];
    };

export interface SortSpec {
  property: string;
  dir: SortDir;
}

export interface BadgeAggregation {
  property: string;
  op: AggregationOp;
}

export interface ViewLayout {
  id: string;
  type: ViewType;
  name: string;
  group_by?: string | null;
  sort?: SortSpec[];
  columns?: string[];
  card_fields?: string[];
  aggregations?: Record<string, AggregationOp>;
  date_property?: string | null;
  end_date_property?: string | null;
  badge_aggregation?: BadgeAggregation | null;
}

export interface ViewSchema {
  id: string;
  name: string;
  source: {
    folder?: string | null;
    filter?: FilterExpr | null;
  };
  properties: Record<string, PropertySchema>;
  views: ViewLayout[];
  default_template?: string | null;
  filename_pattern?: string | null;
}

export interface ViewRow {
  doc_id: string;
  rel_path: string;
  title: string;
  updated_at: string;
  properties: Record<string, PropertyValue>;
  task_completed: number;
  task_total: number;
}

export interface ViewGroupSummary {
  group_key: string;
  count: number;
  aggregations: Record<string, number>;
}

export interface ViewQueryResult {
  view_id: string;
  layout_id: string;
  rows: ViewRow[];
  groups: ViewGroupSummary[];
  aggregations: Record<string, number>;
  total_count: number;
}

export interface SearchHit {
  doc_id: string;
  rel_path: string;
  title: string;
  doc_type: string | null;
  snippet: string;
}

export interface TagCount {
  tag: string;
  count: number;
}

export interface VaultStats {
  vault_path: string;
  doc_count: number;
  tag_count: number;
  link_count: number;
  task_count: number;
}

export interface ReindexReport {
  scanned: number;
  updated: number;
  skipped: number;
  removed: number;
  elapsed_ms: number;
}

export interface TemplateInfo {
  rel_path: string;
  name: string;
}
