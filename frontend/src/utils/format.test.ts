import { describe, expect, it } from 'vitest';

import type { ViewRow, ViewSchema } from '../types/nodikle';
import { formatCurrencyKrw, formatPropertyValue, projectRowToMobileCard } from './format';

describe('format utilities', () => {
  it('formats KRW currency values cleanly', () => {
    expect(formatCurrencyKrw(11000)).toBe('11,000원');
    expect(
      formatPropertyValue(
        { kind: 'number', value: 55000 },
        { label: '금액', type: 'currency', unit: 'KRW' }
      )
    ).toBe('55,000원');
  });

  it('projects a view row into 4-element mobile card fields', () => {
    const schema: ViewSchema = {
      id: 'expenses',
      name: '가계부',
      source: { folder: 'finance' },
      properties: {
        date: { label: '날짜', type: 'date' },
        title: { label: '항목', type: 'text' },
        category: { label: '카테고리', type: 'select', options: ['식비', '교통'] },
        amount: { label: '금액', type: 'currency', unit: 'KRW' },
        payment: { label: '결제수단', type: 'select' },
      },
      views: [],
    };
    const row: ViewRow = {
      doc_id: '1',
      rel_path: 'finance/2026-11-27-lunch.md',
      title: '점심 백반',
      updated_at: '2026-11-27T12:00:00Z',
      properties: {
        date: { kind: 'date', value: '2026-11-27' },
        category: { kind: 'text', value: '식비' },
        amount: { kind: 'number', value: 11000 },
        payment: { kind: 'text', value: '체크카드' },
      },
      task_completed: 0,
      task_total: 0,
    };

    const card = projectRowToMobileCard(row, schema);
    expect(card.title).toBe('점심 백반');
    expect(card.accentMetric).toBe('11,000원');
    expect(card.badge).toBe('식비');
    expect(card.metaItems).toEqual(['2026-11-27', '체크카드']);
  });
});
