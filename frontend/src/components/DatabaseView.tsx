import { useMemo, useState } from 'react';
import {
  Calendar as CalendarIcon,
  ChevronLeft,
  ChevronRight,
  Columns3,
  Filter,
  LayoutGrid,
  Plus,
  Table2,
} from 'lucide-react';

import { useWorkspaceStore } from '../store/useWorkspaceStore';
import type { PropertySchema, ViewLayout, ViewRow, ViewSchema } from '../types/nodikle';
import {
  extractNumber,
  extractText,
  formatCurrencyKrw,
  formatPropertyValue,
  projectRowToMobileCard,
} from '../utils/format';

export function DatabaseView() {
  const {
    views,
    activeViewId,
    activeLayoutId,
    viewResult,
    isMobileViewport,
    openView,
    openDocument,
    patchDocumentProperties,
    setRuntimeFilter,
    setQuickCaptureOpen,
  } = useWorkspaceStore();

  const schema = views.find((v) => v.id === activeViewId) ?? null;
  const layout =
    schema?.views.find((l) => l.id === activeLayoutId) ?? schema?.views[0] ?? null;

  const [compactTableOnMobile, setCompactTableOnMobile] = useState(false);
  const [searchTerm, setSearchTerm] = useState('');
  const [selectedCategory, setSelectedCategory] = useState('전체');
  const [mobileBoardTab, setMobileBoardTab] = useState<string | null>(null);
  const [calendarMonth, setCalendarMonth] = useState(() => {
    const now = new Date();
    return { year: now.getFullYear(), month: now.getMonth() };
  });

  const rows = useMemo(() => {
    if (!viewResult) return [];
    const q = searchTerm.trim().toLowerCase();
    return viewResult.rows.filter((row) => {
      const matchesText =
        !q ||
        row.title.toLowerCase().includes(q) ||
        Object.values(row.properties).some((v) =>
          extractText(v).toLowerCase().includes(q)
        );
      if (!matchesText) return false;
      if (selectedCategory !== '전체') {
        const groupProp = layout?.group_by ?? 'category';
        const val = extractText(row.properties[groupProp]) || '미분류';
        if (val !== selectedCategory) return false;
      }
      return true;
    });
  }, [viewResult, searchTerm, selectedCategory, layout?.group_by]);

  if (!schema || !layout || !viewResult) {
    return <div className="empty-stage">표시할 데이터베이스 뷰가 없습니다.</div>;
  }

  const groupProp = layout.group_by ?? (schema.properties.category ? 'category' : 'status');
  const groupOptions = schema.properties[groupProp]?.options ?? [];

  const aggEntries = Object.entries(viewResult.aggregations);
  const primarySumEntry = aggEntries[0];

  return (
    <div className="db-workspace">
      <header className="db-header">
        <div>
          <h1 className="db-title">{schema.name}</h1>
          <div className="db-sub">
            {viewResult.total_count}개 항목 · {layout.name}
          </div>
        </div>

        <div className="db-actions">
          <div className="layout-tabs">
            {schema.views.map((l) => (
              <button
                key={l.id}
                className={`layout-tab ${l.id === layout.id ? 'active' : ''}`}
                onClick={() => void openView(schema.id, l.id)}
              >
                {l.type === 'table' && <Table2 size={14} />}
                {l.type === 'board' && <Columns3 size={14} />}
                {l.type === 'calendar' && <CalendarIcon size={14} />}
                <span>{l.name}</span>
              </button>
            ))}
          </div>

          <button
            className="btn-primary desktop-only-inline"
            onClick={() => setQuickCaptureOpen(true)}
          >
            <Plus size={14} />
            <span>새 항목</span>
          </button>
        </div>
      </header>

      <div className="db-filter-bar">
        <div className="filter-input-wrap">
          <Filter size={14} />
          <input
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="현재 뷰 내 빠른 필터..."
          />
        </div>

        {groupOptions.length > 0 && (
          <div className="chip-scroll">
            {['전체', ...groupOptions].map((opt) => (
              <button
                key={opt}
                className={`filter-chip ${selectedCategory === opt ? 'active' : ''}`}
                onClick={() => setSelectedCategory(opt)}
              >
                {opt}
              </button>
            ))}
          </div>
        )}

        {schema.properties.date && (
          <button
            className="btn-ghost"
            onClick={() => {
              const ym = `${calendarMonth.year}-${String(calendarMonth.month + 1).padStart(2, '0')}`;
              void setRuntimeFilter({
                op: 'starts_with',
                property: 'date',
                value: ym,
              });
            }}
          >
            이번 달 SQL 필터
          </button>
        )}
        {useWorkspaceStore.getState().runtimeFilter && (
          <button className="btn-ghost" onClick={() => void setRuntimeFilter(null)}>
            SQL 필터 해제
          </button>
        )}

        {layout.type === 'table' && isMobileViewport && (
          <button
            className="btn-ghost"
            onClick={() => setCompactTableOnMobile((prev) => !prev)}
          >
            <LayoutGrid size={14} />
            <span>{compactTableOnMobile ? '카드 뷰' : '컴팩트 표'}</span>
          </button>
        )}
      </div>

      <div className="db-content">
        {layout.type === 'table' &&
          (isMobileViewport && !compactTableOnMobile ? (
            <MobileCardList
              rows={rows}
              schema={schema}
              onOpenDoc={(path) => void openDocument(path)}
            />
          ) : (
            <TableLayoutView
              rows={rows}
              schema={schema}
              layout={layout}
              aggregations={viewResult.aggregations}
              onOpenDoc={(path) => void openDocument(path)}
              onPatchProp={(relPath, key, val) =>
                void patchDocumentProperties(relPath, [{ key, value: val }])
              }
            />
          ))}

        {layout.type === 'board' && (
          <BoardLayoutView
            rows={rows}
            schema={schema}
            layout={layout}
            isMobile={isMobileViewport}
            mobileTab={mobileBoardTab}
            onSelectMobileTab={setMobileBoardTab}
            onOpenDoc={(path) => void openDocument(path)}
            onMoveStatus={(relPath, statusProp, nextStatus) =>
              void patchDocumentProperties(relPath, [
                { key: statusProp, value: { kind: 'text', value: nextStatus } },
              ])
            }
          />
        )}

        {layout.type === 'calendar' && (
          <CalendarLayoutView
            rows={rows}
            schema={schema}
            layout={layout}
            month={calendarMonth}
            onChangeMonth={setCalendarMonth}
            onOpenDoc={(path) => void openDocument(path)}
          />
        )}
      </div>

      {/* Sticky Summary Bar (especially useful on narrow mobile screens) */}
      <footer className="sticky-summary-bar">
        <span>표시 항목: {rows.length}건</span>
        {primarySumEntry && (
          <strong>
            {schema.properties[primarySumEntry[0]]?.label ?? primarySumEntry[0]} 합계:{' '}
            {formatCurrencyKrw(
              rows.reduce(
                (acc, r) => acc + (extractNumber(r.properties[primarySumEntry[0]]) ?? 0),
                0
              )
            )}
          </strong>
        )}
      </footer>
    </div>
  );
}

function MobileCardList({
  rows,
  schema,
  onOpenDoc,
}: {
  rows: ViewRow[];
  schema: ViewSchema;
  onOpenDoc: (relPath: string) => void;
}) {
  if (rows.length === 0) {
    return <div className="empty-hint">조건에 맞는 항목이 없습니다.</div>;
  }
  return (
    <div className="mobile-card-list">
      {rows.map((row) => {
        const card = projectRowToMobileCard(row, schema);
        return (
          <button
            key={row.doc_id}
            className="mobile-card"
            onClick={() => onOpenDoc(row.rel_path)}
          >
            <div className="mobile-card-top">
              <div className="mobile-card-title-wrap">
                {card.badge && <span className="status-pill">{card.badge}</span>}
                <span className="mobile-card-title">{card.title}</span>
              </div>
              {card.accentMetric && (
                <span className="mobile-card-metric">{card.accentMetric}</span>
              )}
            </div>
            {card.metaItems.length > 0 && (
              <div className="mobile-card-meta">
                {card.metaItems.map((m, idx) => (
                  <span key={idx}>{m}</span>
                ))}
              </div>
            )}
          </button>
        );
      })}
    </div>
  );
}

function TableLayoutView({
  rows,
  schema,
  layout,
  aggregations,
  onOpenDoc,
  onPatchProp,
}: {
  rows: ViewRow[];
  schema: ViewSchema;
  layout: ViewLayout;
  aggregations: Record<string, number>;
  onOpenDoc: (relPath: string) => void;
  onPatchProp: (
    relPath: string,
    key: string,
    value: ViewRow['properties'][string]
  ) => void;
}) {
  const cols =
    layout.columns && layout.columns.length > 0
      ? layout.columns
      : Object.keys(schema.properties);

  return (
    <div className="table-scroll">
      <table className="db-table">
        <thead>
          <tr>
            {cols.map((col) => (
              <th key={col}>{schema.properties[col]?.label ?? col}</th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row) => (
            <tr key={row.doc_id}>
              {cols.map((col) => {
                const propSchema: PropertySchema | undefined = schema.properties[col];
                const val = row.properties[col];

                if (col === 'title') {
                  return (
                    <td key={col} className="sticky-col">
                      <button
                        className="row-link"
                        onClick={() => onOpenDoc(row.rel_path)}
                      >
                        {row.title}
                      </button>
                    </td>
                  );
                }

                if (propSchema?.type === 'checkbox') {
                  const checked = val?.kind === 'bool' ? val.value : false;
                  return (
                    <td key={col}>
                      <input
                        type="checkbox"
                        checked={checked}
                        onChange={(e) =>
                          onPatchProp(row.rel_path, col, {
                            kind: 'bool',
                            value: e.target.checked,
                          })
                        }
                      />
                    </td>
                  );
                }

                if (propSchema?.type === 'select' && propSchema.options?.length) {
                  return (
                    <td key={col}>
                      <select
                        className="cell-select"
                        value={extractText(val)}
                        onChange={(e) =>
                          onPatchProp(row.rel_path, col, {
                            kind: 'text',
                            value: e.target.value,
                          })
                        }
                      >
                        <option value="">선택</option>
                        {propSchema.options.map((opt) => (
                          <option key={opt} value={opt}>
                            {opt}
                          </option>
                        ))}
                      </select>
                    </td>
                  );
                }

                return (
                  <td
                    key={col}
                    className={
                      propSchema?.type === 'currency' || propSchema?.type === 'number'
                        ? 'num-cell'
                        : ''
                    }
                  >
                    {formatPropertyValue(val, propSchema)}
                  </td>
                );
              })}
            </tr>
          ))}
        </tbody>
        {Object.keys(aggregations).length > 0 && (
          <tfoot>
            <tr>
              {cols.map((col, idx) => {
                if (idx === 0) {
                  return <td key={col}>합계 ({rows.length}건)</td>;
                }
                if (col in aggregations) {
                  const propSchema = schema.properties[col];
                  const filteredSum = rows.reduce(
                    (acc, r) => acc + (extractNumber(r.properties[col]) ?? 0),
                    0
                  );
                  return (
                    <td key={col} className="num-cell">
                      {formatPropertyValue(
                        { kind: 'number', value: filteredSum },
                        propSchema
                      )}
                    </td>
                  );
                }
                return <td key={col} />;
              })}
            </tr>
          </tfoot>
        )}
      </table>
    </div>
  );
}

function BoardLayoutView({
  rows,
  schema,
  layout,
  isMobile,
  mobileTab,
  onSelectMobileTab,
  onOpenDoc,
  onMoveStatus,
}: {
  rows: ViewRow[];
  schema: ViewSchema;
  layout: ViewLayout;
  isMobile: boolean;
  mobileTab: string | null;
  onSelectMobileTab: (tab: string) => void;
  onOpenDoc: (relPath: string) => void;
  onMoveStatus: (relPath: string, statusProp: string, nextStatus: string) => void;
}) {
  const groupProp = layout.group_by ?? 'status';
  const configuredOpts = schema.properties[groupProp]?.options ?? [];
  const columnKeys = Array.from(
    new Set([
      ...configuredOpts,
      ...rows.map((r) => extractText(r.properties[groupProp]) || '미분류'),
    ])
  );

  const activeMobileCol =
    mobileTab && columnKeys.includes(mobileTab) ? mobileTab : columnKeys[0] ?? '미분류';
  const visibleColumns = isMobile ? [activeMobileCol] : columnKeys;

  return (
    <div className="board-wrap">
      {isMobile && (
        <div className="board-segment-tabs">
          {columnKeys.map((col) => {
            const count = rows.filter(
              (r) => (extractText(r.properties[groupProp]) || '미분류') === col
            ).length;
            return (
              <button
                key={col}
                className={col === activeMobileCol ? 'active' : ''}
                onClick={() => onSelectMobileTab(col)}
              >
                {col} ({count})
              </button>
            );
          })}
        </div>
      )}

      <div className="board-columns">
        {visibleColumns.map((col) => {
          const colRows = rows.filter(
            (r) => (extractText(r.properties[groupProp]) || '미분류') === col
          );
          return (
            <div
              key={col}
              className="board-column"
              onDragOver={(e) => e.preventDefault()}
              onDrop={(e) => {
                const relPath = e.dataTransfer.getData('text/plain');
                if (relPath) onMoveStatus(relPath, groupProp, col);
              }}
            >
              <div className="board-col-header">
                <span>{col}</span>
                <span className="col-count">{colRows.length}</span>
              </div>
              <div className="board-cards">
                {colRows.map((row) => {
                  const pct =
                    row.task_total > 0
                      ? Math.round((row.task_completed / row.task_total) * 100)
                      : null;
                  return (
                    <div
                      key={row.doc_id}
                      className="board-card"
                      draggable
                      onDragStart={(e) =>
                        e.dataTransfer.setData('text/plain', row.rel_path)
                      }
                    >
                      <button
                        className="board-card-title"
                        onClick={() => onOpenDoc(row.rel_path)}
                      >
                        {row.title}
                      </button>

                      <div className="board-card-fields">
                        {(layout.card_fields ?? []).map((field) => {
                          const val = row.properties[field];
                          if (!val) return null;
                          return (
                            <div key={field} className="board-field-row">
                              <span>{schema.properties[field]?.label ?? field}</span>
                              <span>
                                {formatPropertyValue(val, schema.properties[field])}
                              </span>
                            </div>
                          );
                        })}
                      </div>

                      {pct !== null && (
                        <div className="progress-wrap">
                          <div className="progress-bar">
                            <div
                              className="progress-fill"
                              style={{ width: `${pct}%` }}
                            />
                          </div>
                          <span>
                            {row.task_completed}/{row.task_total} ({pct}%)
                          </span>
                        </div>
                      )}

                      {configuredOpts.length > 0 && (
                        <select
                          className="board-move-select"
                          value={col}
                          onChange={(e) =>
                            onMoveStatus(row.rel_path, groupProp, e.target.value)
                          }
                        >
                          {configuredOpts.map((opt) => (
                            <option key={opt} value={opt}>
                              상태: {opt}
                            </option>
                          ))}
                        </select>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}

function CalendarLayoutView({
  rows,
  schema,
  layout,
  month,
  onChangeMonth,
  onOpenDoc,
}: {
  rows: ViewRow[];
  schema: ViewSchema;
  layout: ViewLayout;
  month: { year: number; month: number };
  onChangeMonth: (next: { year: number; month: number }) => void;
  onOpenDoc: (relPath: string) => void;
}) {
  const dateProp = layout.date_property ?? 'date';
  const firstDay = new Date(month.year, month.month, 1);
  const daysInMonth = new Date(month.year, month.month + 1, 0).getDate();
  const startOffset = firstDay.getDay();

  const shiftMonth = (delta: number) => {
    const next = new Date(month.year, month.month + delta, 1);
    onChangeMonth({ year: next.getFullYear(), month: next.getMonth() });
  };

  const cells: (number | null)[] = [
    ...Array.from({ length: startOffset }, () => null),
    ...Array.from({ length: daysInMonth }, (_, i) => i + 1),
  ];

  const badgeProp = layout.badge_aggregation?.property;

  return (
    <div className="calendar-wrap">
      <div className="calendar-nav">
        <button className="icon-btn" onClick={() => shiftMonth(-1)}>
          <ChevronLeft size={16} />
        </button>
        <strong>
          {month.year}년 {month.month + 1}월
        </strong>
        <button className="icon-btn" onClick={() => shiftMonth(1)}>
          <ChevronRight size={16} />
        </button>
      </div>

      <div className="calendar-grid">
        {['일', '월', '화', '수', '목', '금', '토'].map((d) => (
          <div key={d} className="cal-weekday">
            {d}
          </div>
        ))}
        {cells.map((day, idx) => {
          if (day === null) {
            return <div key={`empty-${idx}`} className="cal-cell empty" />;
          }
          const isoDay = `${month.year}-${String(month.month + 1).padStart(2, '0')}-${String(
            day
          ).padStart(2, '0')}`;
          const dayRows = rows.filter((r) =>
            extractText(r.properties[dateProp]).startsWith(isoDay)
          );
          const daySum = badgeProp
            ? dayRows.reduce(
                (acc, r) => acc + (extractNumber(r.properties[badgeProp]) ?? 0),
                0
              )
            : 0;

          return (
            <div key={isoDay} className="cal-cell">
              <div className="cal-day-head">
                <span>{day}</span>
                {badgeProp && daySum > 0 && (
                  <span className="cal-badge">
                    {formatPropertyValue(
                      { kind: 'number', value: daySum },
                      schema.properties[badgeProp]
                    )}
                  </span>
                )}
              </div>
              <div className="cal-items">
                {dayRows.map((r) => (
                  <button
                    key={r.doc_id}
                    className="cal-item"
                    onClick={() => onOpenDoc(r.rel_path)}
                  >
                    {r.title}
                  </button>
                ))}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
