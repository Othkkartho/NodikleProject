import { useEffect, useState } from 'react';
import { X } from 'lucide-react';

import { useWorkspaceStore } from '../store/useWorkspaceStore';
import type { PropertyValue } from '../types/nodikle';
import { todayIsoDate } from '../utils/format';

export function QuickCaptureSheet() {
  const {
    views,
    activeViewId,
    quickCaptureOpen,
    setQuickCaptureOpen,
    createFromTemplate,
  } = useWorkspaceStore();

  const schema = views.find((v) => v.id === activeViewId) ?? views[0] ?? null;
  const [title, setTitle] = useState('');
  const [fieldValues, setFieldValues] = useState<Record<string, string>>({});

  useEffect(() => {
    if (!quickCaptureOpen || !schema) return;
    const defaults: Record<string, string> = {};
    for (const [key, prop] of Object.entries(schema.properties)) {
      if (prop.type === 'date') {
        defaults[key] = todayIsoDate();
      } else if (prop.type === 'select' && prop.options?.length) {
        defaults[key] = prop.options[0];
      } else {
        defaults[key] = '';
      }
    }
    setTitle('');
    setFieldValues(defaults);
  }, [quickCaptureOpen, schema?.id]);

  if (!quickCaptureOpen || !schema) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    const vars: Record<string, PropertyValue> = {};
    for (const [key, prop] of Object.entries(schema.properties)) {
      if (key === 'title') continue;
      const raw = (fieldValues[key] ?? '').trim();
      if (!raw) continue;
      if (prop.type === 'number' || prop.type === 'currency') {
        const n = Number(raw);
        if (Number.isFinite(n)) {
          vars[key] = { kind: 'number', value: n };
        }
      } else if (prop.type === 'date') {
        vars[key] = { kind: 'date', value: raw };
      } else if (prop.type === 'checkbox') {
        vars[key] = { kind: 'bool', value: raw === 'true' };
      } else {
        vars[key] = { kind: 'text', value: raw };
      }
    }
    void createFromTemplate(title.trim() || '새 항목', vars, schema.id);
  };

  return (
    <div className="sheet-backdrop" onClick={() => setQuickCaptureOpen(false)}>
      <div className="bottom-sheet" onClick={(e) => e.stopPropagation()}>
        <div className="sheet-header">
          <strong>{schema.name} 빠른 입력</strong>
          <button
            className="icon-btn"
            aria-label="닫기"
            onClick={() => setQuickCaptureOpen(false)}
          >
            <X size={18} />
          </button>
        </div>

        <form className="sheet-form" onSubmit={handleSubmit}>
          <label className="field-label">
            <span>제목 / 항목명</span>
            <input
              autoFocus
              required
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="예: 저녁 식사, 제주도 주말 여행"
            />
          </label>

          {Object.entries(schema.properties)
            .filter(([k]) => k !== 'title' && k !== 'settled')
            .map(([key, prop]) => {
              if (prop.type === 'select' && prop.options?.length) {
                return (
                  <div key={key} className="field-label">
                    <span>{prop.label}</span>
                    <div className="chip-scroll">
                      {prop.options.map((opt) => (
                        <button
                          type="button"
                          key={opt}
                          className={`filter-chip ${
                            fieldValues[key] === opt ? 'active' : ''
                          }`}
                          onClick={() =>
                            setFieldValues((prev) => ({ ...prev, [key]: opt }))
                          }
                        >
                          {opt}
                        </button>
                      ))}
                    </div>
                  </div>
                );
              }

              return (
                <label key={key} className="field-label">
                  <span>{prop.label}</span>
                  <input
                    type={prop.type === 'date' ? 'date' : 'text'}
                    inputMode={
                      prop.type === 'currency' || prop.type === 'number'
                        ? 'decimal'
                        : undefined
                    }
                    value={fieldValues[key] ?? ''}
                    onChange={(e) =>
                      setFieldValues((prev) => ({ ...prev, [key]: e.target.value }))
                    }
                    placeholder={
                      prop.type === 'currency' ? '금액 입력 (숫자)' : `${prop.label} 입력`
                    }
                  />
                </label>
              );
            })}

          <div className="sheet-actions">
            <button
              type="button"
              className="btn-ghost"
              onClick={() => setQuickCaptureOpen(false)}
            >
              취소
            </button>
            <button type="submit" className="btn-primary">
              저장하기
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
