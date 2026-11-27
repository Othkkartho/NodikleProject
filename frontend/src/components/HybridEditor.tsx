import { useEffect, useMemo, useRef, useState } from 'react';
import { markdown } from '@codemirror/lang-markdown';
import { EditorState } from '@codemirror/state';
import { EditorView, keymap } from '@codemirror/view';
import TaskItem from '@tiptap/extension-task-item';
import TaskList from '@tiptap/extension-task-list';
import Placeholder from '@tiptap/extension-placeholder';
import { EditorContent, useEditor } from '@tiptap/react';
import StarterKit from '@tiptap/starter-kit';
import {
  ArrowLeft,
  CheckSquare,
  Code2,
  FileCode2,
  Link2,
  Plus,
  Save,
  Trash2,
} from 'lucide-react';

import { useWorkspaceStore } from '../store/useWorkspaceStore';
import type { PropertyValue } from '../types/nodikle';
import { extractText } from '../utils/format';

type EditorMode = 'raw' | 'blocks';

function markdownToSimpleHtml(md: string): string {
  const lines = md.split(/\r?\n/);
  const htmlParts: string[] = [];
  for (const line of lines) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    if (trimmed.startsWith('### ')) {
      htmlParts.push(`<h3>${escapeHtml(trimmed.slice(4))}</h3>`);
    } else if (trimmed.startsWith('## ')) {
      htmlParts.push(`<h2>${escapeHtml(trimmed.slice(3))}</h2>`);
    } else if (trimmed.startsWith('# ')) {
      htmlParts.push(`<h1>${escapeHtml(trimmed.slice(2))}</h1>`);
    } else if (trimmed.startsWith('- [x] ') || trimmed.startsWith('- [X] ')) {
      htmlParts.push(
        `<ul data-type="taskList"><li data-type="taskItem" data-checked="true"><p>${escapeHtml(trimmed.slice(6))}</p></li></ul>`
      );
    } else if (trimmed.startsWith('- [ ] ')) {
      htmlParts.push(
        `<ul data-type="taskList"><li data-type="taskItem" data-checked="false"><p>${escapeHtml(trimmed.slice(6))}</p></li></ul>`
      );
    } else if (trimmed.startsWith('- ')) {
      htmlParts.push(`<ul><li><p>${escapeHtml(trimmed.slice(2))}</p></li></ul>`);
    } else {
      htmlParts.push(`<p>${escapeHtml(trimmed)}</p>`);
    }
  }
  return htmlParts.join('') || '<p></p>';
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;');
}

function tiptapJsonToMarkdown(doc: ReturnType<NonNullable<ReturnType<typeof useEditor>>['getJSON']>): string {
  const lines: string[] = [];
  for (const node of doc.content ?? []) {
    const text = (node.content ?? []).map((c) => c.text ?? '').join('');
    if (node.type === 'heading') {
      const level = Number(node.attrs?.level ?? 1);
      lines.push(`${'#'.repeat(level)} ${text}`);
    } else if (node.type === 'taskList') {
      for (const item of node.content ?? []) {
        const checked = Boolean(item.attrs?.checked);
        const itemText = (item.content ?? [])
          .flatMap((p) => p.content ?? [])
          .map((c) => c.text ?? '')
          .join('');
        lines.push(`- [${checked ? 'x' : ' '}] ${itemText}`);
      }
    } else if (node.type === 'bulletList') {
      for (const item of node.content ?? []) {
        const itemText = (item.content ?? [])
          .flatMap((p) => p.content ?? [])
          .map((c) => c.text ?? '')
          .join('');
        lines.push(`- ${itemText}`);
      }
    } else if (node.type === 'codeBlock') {
      lines.push('```');
      lines.push(text);
      lines.push('```');
    } else {
      lines.push(text);
    }
    lines.push('');
  }
  return lines.join('\n').trimEnd() + '\n';
}

export function HybridEditor() {
  const {
    activeDoc,
    documents,
    views,
    activeViewId,
    saveActiveDocument,
    patchDocumentProperties,
    toggleTask,
    deleteDocument,
    openWikilink,
    openView,
  } = useWorkspaceStore();

  const [mode, setMode] = useState<EditorMode>('raw');
  const [draftRaw, setDraftRaw] = useState('');
  const [dirty, setDirty] = useState(false);
  const [wikiQuery, setWikiQuery] = useState<string | null>(null);
  const [newPropKey, setNewPropKey] = useState('');
  const [newPropVal, setNewPropVal] = useState('');

  const cmHostRef = useRef<HTMLDivElement | null>(null);
  const cmViewRef = useRef<EditorView | null>(null);

  const blockEditor = useEditor({
    extensions: [
      StarterKit,
      TaskList,
      TaskItem.configure({ nested: true }),
      Placeholder.configure({ placeholder: '블록 내용을 입력하세요...' }),
    ],
    content: '<p></p>',
    onUpdate: ({ editor }) => {
      if (!activeDoc || mode !== 'blocks') return;
      const bodyMd = tiptapJsonToMarkdown(editor.getJSON());
      const fmEnd = activeDoc.raw_md.indexOf(activeDoc.body_md);
      const fmPrefix = fmEnd > 0 ? activeDoc.raw_md.slice(0, fmEnd) : '';
      setDraftRaw(fmPrefix + bodyMd);
      setDirty(true);
    },
  });

  useEffect(() => {
    if (!activeDoc) return;
    setDraftRaw(activeDoc.raw_md);
    setDirty(false);
    setWikiQuery(null);
    if (blockEditor) {
      blockEditor.commands.setContent(markdownToSimpleHtml(activeDoc.body_md), false);
    }
  }, [activeDoc?.meta.rel_path, activeDoc?.meta.content_hash]);

  // Mount CodeMirror 6 in raw Markdown mode.
  useEffect(() => {
    if (mode !== 'raw' || !cmHostRef.current || !activeDoc) {
      return;
    }

    const saveBinding = keymap.of([
      {
        key: 'Mod-s',
        run: () => {
          void saveActiveDocument(cmViewRef.current?.state.doc.toString() ?? draftRaw);
          setDirty(false);
          return true;
        },
      },
    ]);

    const state = EditorState.create({
      doc: draftRaw,
      extensions: [
        markdown(),
        EditorView.lineWrapping,
        saveBinding,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            const next = update.state.doc.toString();
            setDraftRaw(next);
            setDirty(true);

            // Detect `[[...` at cursor for wikilink autocomplete.
            const pos = update.state.selection.main.head;
            const before = next.slice(Math.max(0, pos - 40), pos);
            const match = /\[\[([^\]\n]*)$/.exec(before);
            setWikiQuery(match ? match[1] : null);
          }
        }),
      ],
    });

    const view = new EditorView({
      state,
      parent: cmHostRef.current,
    });
    cmViewRef.current = view;

    return () => {
      view.destroy();
      cmViewRef.current = null;
    };
  }, [mode, activeDoc?.meta.rel_path, activeDoc?.meta.content_hash]);

  const wikiSuggestions = useMemo(() => {
    if (wikiQuery === null) return [];
    const q = wikiQuery.trim().toLowerCase();
    return documents
      .filter(
        (d) =>
          d.stem.toLowerCase().includes(q) || d.title.toLowerCase().includes(q)
      )
      .slice(0, 6);
  }, [wikiQuery, documents]);

  if (!activeDoc) {
    return (
      <div className="empty-stage">
        사이드바에서 문서를 선택하거나 새 노트를 만들어 주세요.
      </div>
    );
  }

  const insertWikilink = (stem: string) => {
    const cm = cmViewRef.current;
    if (!cm || wikiQuery === null) return;
    const pos = cm.state.selection.main.head;
    const from = pos - wikiQuery.length;
    cm.dispatch({
      changes: { from, to: pos, insert: `${stem}]]` },
      selection: { anchor: from + stem.length + 2 },
    });
    setWikiQuery(null);
  };

  const handleSave = () => {
    void saveActiveDocument(draftRaw);
    setDirty(false);
  };

  const handlePropertyUpdate = (key: string, rawVal: string, kind: PropertyValue['kind']) => {
    let value: PropertyValue;
    if (kind === 'number') {
      const n = Number(rawVal);
      value = { kind: 'number', value: Number.isFinite(n) ? n : 0 };
    } else if (kind === 'bool') {
      value = { kind: 'bool', value: rawVal === 'true' };
    } else if (kind === 'date') {
      value = { kind: 'date', value: rawVal };
    } else {
      value = { kind: 'text', value: rawVal };
    }
    void patchDocumentProperties(activeDoc.meta.rel_path, [{ key, value }]);
  };

  const handleAddProperty = () => {
    const key = newPropKey.trim();
    if (!key) return;
    const num = Number(newPropVal);
    const val: PropertyValue =
      newPropVal.trim() !== '' && Number.isFinite(num)
        ? { kind: 'number', value: num }
        : { kind: 'text', value: newPropVal.trim() };
    void patchDocumentProperties(activeDoc.meta.rel_path, [{ key, value: val }]);
    setNewPropKey('');
    setNewPropVal('');
  };

  return (
    <div className="editor-workspace">
      <header className="editor-toolbar">
        <div className="toolbar-left">
          {views.length > 0 && (
            <button
              className="btn-ghost"
              onClick={() => void openView(activeViewId ?? views[0].id)}
            >
              <ArrowLeft size={15} />
              <span>뷰로 돌아가기</span>
            </button>
          )}
          <div className="doc-path-chip">{activeDoc.meta.rel_path}</div>
        </div>

        <div className="toolbar-right">
          <div className="segmented-control">
            <button
              className={mode === 'raw' ? 'active' : ''}
              onClick={() => setMode('raw')}
            >
              <FileCode2 size={14} />
              <span>Markdown</span>
            </button>
            <button
              className={mode === 'blocks' ? 'active' : ''}
              onClick={() => {
                if (blockEditor) {
                  const splitBody = draftRaw.replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/, '');
                  blockEditor.commands.setContent(markdownToSimpleHtml(splitBody), false);
                }
                setMode('blocks');
              }}
            >
              <Code2 size={14} />
              <span>블록 편집</span>
            </button>
          </div>

          <button
            className={`btn-primary ${dirty ? 'pulse' : ''}`}
            onClick={handleSave}
            disabled={!dirty}
          >
            <Save size={14} />
            <span>{dirty ? '저장' : '저장됨'}</span>
          </button>
          <button
            className="btn-danger-ghost"
            title="문서 삭제"
            onClick={() => void deleteDocument(activeDoc.meta.rel_path)}
          >
            <Trash2 size={15} />
          </button>
        </div>
      </header>

      <div className="editor-scroll">
        <h1 className="doc-heading">{activeDoc.meta.title}</h1>

        {/* YAML Frontmatter Property Inspector */}
        <section className="property-panel">
          <div className="panel-subtitle">문서 속성 (Frontmatter)</div>
          <div className="property-grid">
            {activeDoc.ordered_keys
              .filter((k) => k !== 'id')
              .map((key) => {
                const val = activeDoc.properties[key];
                if (!val) return null;
                if (val.kind === 'bool') {
                  return (
                    <div key={key} className="prop-row">
                      <span className="prop-key">{key}</span>
                      <label className="checkbox-inline">
                        <input
                          type="checkbox"
                          checked={val.value}
                          onChange={(e) =>
                            handlePropertyUpdate(key, String(e.target.checked), 'bool')
                          }
                        />
                        <span>{val.value ? 'true' : 'false'}</span>
                      </label>
                    </div>
                  );
                }
                return (
                  <div key={key} className="prop-row">
                    <span className="prop-key">{key}</span>
                    <input
                      className="prop-input"
                      defaultValue={extractText(val)}
                      onBlur={(e) => {
                        if (e.target.value !== extractText(val)) {
                          handlePropertyUpdate(key, e.target.value, val.kind);
                        }
                      }}
                    />
                  </div>
                );
              })}
          </div>
          <div className="prop-add-row">
            <input
              placeholder="새 속성 키"
              value={newPropKey}
              onChange={(e) => setNewPropKey(e.target.value)}
            />
            <input
              placeholder="값"
              value={newPropVal}
              onChange={(e) => setNewPropVal(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === 'Enter') handleAddProperty();
              }}
            />
            <button className="btn-ghost" onClick={handleAddProperty}>
              <Plus size={14} />
              <span>속성 추가</span>
            </button>
          </div>
        </section>

        {/* Hybrid Editor Surface */}
        <section className="editor-surface">
          {mode === 'raw' ? (
            <div className="cm-wrapper">
              <div ref={cmHostRef} className="cm-host" />
              {wikiQuery !== null && wikiSuggestions.length > 0 && (
                <div className="wikilink-popover">
                  <div className="popover-title">문서 링크 자동완성</div>
                  {wikiSuggestions.map((doc) => (
                    <button
                      key={doc.id}
                      className="popover-item"
                      onClick={() => insertWikilink(doc.stem)}
                    >
                      <span>[[{doc.stem}]]</span>
                      <span className="popover-sub">{doc.title}</span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <div className="tiptap-wrapper">
              <EditorContent editor={blockEditor} />
            </div>
          )}
        </section>

        {/* Interactive Checklist Panel */}
        {activeDoc.tasks.length > 0 && (
          <section className="meta-card">
            <div className="panel-subtitle">
              <CheckSquare size={14} />
              <span>
                체크리스트 (
                {activeDoc.tasks.filter((t) => t.completed).length}/
                {activeDoc.tasks.length})
              </span>
            </div>
            <ul className="task-list">
              {activeDoc.tasks.map((task) => (
                <li key={task.line_no}>
                  <label className="task-row">
                    <input
                      type="checkbox"
                      checked={task.completed}
                      onChange={(e) =>
                        void toggleTask(
                          activeDoc.meta.rel_path,
                          task.line_no,
                          e.target.checked
                        )
                      }
                    />
                    <span className={task.completed ? 'task-done' : ''}>
                      {task.body}
                    </span>
                  </label>
                </li>
              ))}
            </ul>
          </section>
        )}

        {/* Outgoing Links & Backlinks */}
        <section className="links-split">
          <div className="meta-card">
            <div className="panel-subtitle">
              <Link2 size={14} />
              <span>나가는 링크 ({activeDoc.wikilinks.length})</span>
            </div>
            {activeDoc.wikilinks.length === 0 ? (
              <div className="empty-hint">본문에 [[위키링크]]가 없습니다.</div>
            ) : (
              <div className="link-chip-list">
                {activeDoc.wikilinks.map((link, idx) => (
                  <button
                    key={`${link.raw_target}-${idx}`}
                    className="link-chip"
                    onClick={() => void openWikilink(link.raw_target)}
                  >
                    {link.alias ? `${link.alias} (${link.raw_target})` : link.raw_target}
                  </button>
                ))}
              </div>
            )}
          </div>

          <div className="meta-card">
            <div className="panel-subtitle">
              <Link2 size={14} />
              <span>백링크 ({activeDoc.backlinks.length})</span>
            </div>
            {activeDoc.backlinks.length === 0 ? (
              <div className="empty-hint">이 문서를 참조하는 다른 문서가 없습니다.</div>
            ) : (
              <div className="link-chip-list">
                {activeDoc.backlinks.map((bl) => (
                  <button
                    key={`${bl.source_doc_id}-${bl.raw_target}`}
                    className="link-chip"
                    onClick={() => void openDocumentLink(bl.source_rel_path)}
                  >
                    {bl.source_title} ({bl.source_rel_path})
                  </button>
                ))}
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}

function openDocumentLink(relPath: string) {
  return useWorkspaceStore.getState().openDocument(relPath);
}
