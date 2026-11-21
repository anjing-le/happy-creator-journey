import { KeyboardEvent, useEffect, useId, useLayoutEffect, useRef, useState } from 'react';
import { createPortal } from 'react-dom';

export interface ChoiceOption { value: string; label: string; group?: string }
interface Props {
  label: string; value: string; options: ChoiceOption[]; onChange: (value: string) => void;
  className?: string; disabled?: boolean; editable?: boolean; placeholder?: string;
}

/** App-owned listbox, shared by session, Skill and editable category selection. */
export function Choice({ label, value, options, onChange, className = '', disabled = false, editable = false, placeholder }: Props) {
  const id = useId();
  const anchor = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement | HTMLInputElement>(null);
  const popup = useRef<HTMLDivElement>(null);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const [position, setPosition] = useState<{ left: number; top?: number; bottom?: number; width: number; maxHeight: number }>({ left: 0, top: 0, width: 0, maxHeight: 280 });
  const typeahead = useRef({ text: '', time: 0 });
  const items = editable ? options.filter(o => o.label.toLocaleLowerCase().includes(value.toLocaleLowerCase())) : options;
  const shown = open && !disabled && items.length > 0;
  const selected = options.find(o => o.value === value);
  function expand() { trigger.current?.focus(); setActive(Math.max(0, items.findIndex(o => o.value === value))); setOpen(true); }
  function choose(index: number) {
    const item = items[index]; if (!item) return;
    onChange(item.value); setOpen(false); trigger.current?.focus();
  }
  useLayoutEffect(() => {
    if (!shown) return;
    const place = () => {
      const r = anchor.current!.getBoundingClientRect();
      const below = window.innerHeight - r.bottom - 16;
      const above = r.top - 16;
      const upward = below < 220 && above > below;
      const maxHeight = Math.min(320, Math.max(64, upward ? above : below));
      const width = Math.min(window.innerWidth - 24, Math.max(r.width, editable ? 240 : 300));
      setPosition({ left: Math.max(12, Math.min(r.left, window.innerWidth - width - 12)),
        ...(upward ? { bottom: window.innerHeight - r.top + 7 } : { top: r.bottom + 7 }), width, maxHeight });
    };
    place(); window.addEventListener('resize', place);
    const scroll = (event: Event) => { if (!popup.current?.contains(event.target as Node)) place(); };
    window.addEventListener('scroll', scroll, true);
    return () => { window.removeEventListener('resize', place); window.removeEventListener('scroll', scroll, true); };
  }, [shown, items.length]);
  useEffect(() => {
    if (!open) return;
    const outside = (event: Event) => {
      const target = event.target as Node;
      if (!anchor.current?.contains(target) && !popup.current?.contains(target)) setOpen(false);
    };
    document.addEventListener('pointerdown', outside); document.addEventListener('focusin', outside);
    return () => { document.removeEventListener('pointerdown', outside); document.removeEventListener('focusin', outside); };
  }, [open]);
  useEffect(() => { popup.current?.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' }); }, [active, shown]);
  function keys(event: KeyboardEvent) {
    if (event.nativeEvent.isComposing) return;
    if (event.key === 'Escape') { if (open) { event.preventDefault(); event.stopPropagation(); setOpen(false); } return; }
    if (event.key === 'Tab') { setOpen(false); return; }
    if (['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
      if (editable && ['Home', 'End'].includes(event.key)) return;
      event.preventDefault();
      if (!shown) { expand(); return; }
      setActive(old => event.key === 'Home' ? 0 : event.key === 'End' ? items.length - 1 : (old + (event.key === 'ArrowUp' ? -1 : 1) + items.length) % items.length);
    } else if (event.key === 'Enter' || (!editable && event.key === ' ')) {
      event.preventDefault(); if (shown) choose(active); else if (!editable) expand();
    } else if (!editable && event.key.length === 1 && !event.metaKey && !event.ctrlKey && !event.altKey) {
      const now = Date.now(); const text = now - typeahead.current.time < 600 ? typeahead.current.text + event.key : event.key;
      typeahead.current = { text, time: now };
      const index = items.findIndex(o => o.label.toLocaleLowerCase().startsWith(text.toLocaleLowerCase()));
      if (index >= 0) { event.preventDefault(); setOpen(true); setActive(index); }
    }
  }
  const a11y = { role: 'combobox', 'aria-label': label, 'aria-expanded': shown, 'aria-controls': shown ? id : undefined,
    'aria-activedescendant': shown && items[active] ? `${id}-${active}` : undefined, 'aria-haspopup': 'listbox' as const };
  return <div className={`choice ${className}`} ref={anchor}>
    {editable ? <input {...a11y} ref={element => { trigger.current = element; }} disabled={disabled} value={value} maxLength={80} placeholder={placeholder}
      autoComplete="off" onFocus={expand} onClick={expand} onKeyDown={keys} onChange={e => { onChange(e.target.value); setActive(0); setOpen(true); }} />
      : <button {...a11y} ref={element => { trigger.current = element; }} className="choice-trigger" type="button" disabled={disabled} onKeyDown={keys} onClick={() => shown ? setOpen(false) : expand()}>
        <span>{selected?.label || placeholder || label}</span><svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4 6 4 4 4-4" /></svg>
      </button>}
    {shown && createPortal(<div id={id} ref={popup} role="listbox" aria-label={label} className="choice-popup" style={position} onPointerDown={e => e.preventDefault()}>
      {items.map((item, index) => <div key={item.value}>
        {item.group && item.group !== items[index - 1]?.group && <div className="choice-group" role="presentation">{item.group}</div>}
        <div id={`${id}-${index}`} role="option" aria-selected={item.value === value} className="choice-option" data-active={index === active} onPointerMove={() => setActive(index)} onClick={() => choose(index)}>
          <span className="choice-check" aria-hidden="true">{item.value === value ? '✓' : ''}</span><span>{item.label}</span>
        </div>
      </div>)}
    </div>, document.body)}
  </div>;
}
