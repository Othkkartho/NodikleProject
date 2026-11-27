import type { PropertySchema, PropertyValue, ViewRow, ViewSchema } from '../types/nodikle';

export function extractText(val?: PropertyValue): string {
  if (!val || val.kind === 'null') return '';
  if (val.kind === 'list') return val.value.join(', ');
  return String(val.value);
}

export function extractNumber(val?: PropertyValue): number | null {
  if (!val) return null;
  if (val.kind === 'number') return val.value;
  if (val.kind === 'text') {
    const parsed = Number(val.value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

export function formatPropertyValue(val: PropertyValue | undefined, schema?: PropertySchema): string {
  if (!val || val.kind === 'null') return '—';
  if (val.kind === 'bool') return val.value ? '완료' : '미완료';
  if (val.kind === 'list') return val.value.join(', ');
  if (val.kind === 'number') {
    const formatted = new Intl.NumberFormat('ko-KR').format(val.value);
    if (schema?.type === 'currency') {
      return schema.unit === 'KRW' || !schema.unit ? `${formatted}원` : `${formatted} ${schema.unit}`;
    }
    return formatted;
  }
  return String(val.value);
}

export function formatCurrencyKrw(amount: number): string {
  return `${new Intl.NumberFormat('ko-KR').format(Math.round(amount))}원`;
}

export function todayIsoDate(): string {
  const now = new Date();
  const y = now.getFullYear();
  const m = String(now.getMonth() + 1).padStart(2, '0');
  const d = String(now.getDate()).padStart(2, '0');
  return `${y}-${m}-${d}`;
}

export interface MobileCardProjection {
  title: string;
  accentMetric: string | null;
  badge: string | null;
  metaItems: string[];
}

export function projectRowToMobileCard(row: ViewRow, schema: ViewSchema): MobileCardProjection {
  const propEntries = Object.entries(schema.properties);
  const currencyEntry = propEntries.find(([, s]) => s.type === 'currency' || s.type === 'number');
  const badgeEntry = propEntries.find(([, s]) => s.type === 'select');
  const secondaryEntries = propEntries
    .filter(
      ([k, s]) =>
        k !== 'title' &&
        k !== currencyEntry?.[0] &&
        k !== badgeEntry?.[0] &&
        (s.type === 'date' || s.type === 'text' || s.type === 'select')
    )
    .slice(0, 2);

  const accentMetric = currencyEntry
    ? formatPropertyValue(row.properties[currencyEntry[0]], currencyEntry[1])
    : null;
  const badge = badgeEntry ? extractText(row.properties[badgeEntry[0]]) || null : null;

  const metaItems = secondaryEntries
    .map(([k, s]) => formatPropertyValue(row.properties[k], s))
    .filter((v) => v && v !== '—');

  if (row.task_total > 0) {
    metaItems.push(`체크 ${row.task_completed}/${row.task_total}`);
  }

  return {
    title: row.title,
    accentMetric: accentMetric === '—' ? null : accentMetric,
    badge,
    metaItems: metaItems.slice(0, 2),
  };
}
