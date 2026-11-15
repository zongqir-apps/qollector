import { memo, useState, useEffect, useRef, useMemo, useCallback } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Pencil, X, Check } from 'lucide-react';
import '../../styles/quick-note-popover.css';

const POPOVER_WIDTH = 320;
const POPOVER_HEIGHT = 104;
const VIEWPORT_PADDING = 12;
const GAP = 6;

function QuickNotePopover({
  item,
  anchorPosition,
  isMiniMode = false,
  onClose,
  onSaveNote,
}) {
  const { t } = useTranslation();
  const popoverRef = useRef(null);
  const inputRef = useRef(null);
  const [noteInput, setNoteInput] = useState(() => (typeof item?.user_note === 'string' ? item.user_note : ''));
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    const frame = requestAnimationFrame(() => {
      if (inputRef.current) {
        inputRef.current.focus();
        inputRef.current.select();
      }
    });
    return () => cancelAnimationFrame(frame);
  }, []);

  const handleSaveAndClose = useCallback(async () => {
    if (busy || !item) return;
    const trimmed = noteInput.trim();
    setBusy(true);
    try {
      await onSaveNote?.(item, trimmed);
      onClose();
    } catch (err) {
      console.error('[QuickNotePopover] Failed to save note:', err);
    } finally {
      setBusy(false);
    }
  }, [busy, item, noteInput, onClose, onSaveNote]);

  const handleKeyDown = useCallback((event) => {
    if (event.key === 'Escape') {
      event.preventDefault();
      event.stopPropagation();
      onClose();
    } else if (event.key === 'Enter') {
      event.preventDefault();
      event.stopPropagation();
      void handleSaveAndClose();
    }
  }, [handleSaveAndClose, onClose]);

  useEffect(() => {
    const handleClickOutside = (event) => {
      if (popoverRef.current && !popoverRef.current.contains(event.target)) {
        onClose();
      }
    };
    window.addEventListener('pointerdown', handleClickOutside, true);
    return () => window.removeEventListener('pointerdown', handleClickOutside, true);
  }, [onClose]);

  const { positionStyle, placement } = useMemo(() => {
    if (typeof window === 'undefined') {
      return { positionStyle: {}, placement: 'bottom' };
    }
    const winWidth = window.innerWidth || 800;
    const winHeight = window.innerHeight || 600;

    if (isMiniMode || winWidth <= 420) {
      return {
        positionStyle: {
          position: 'fixed',
          bottom: `${VIEWPORT_PADDING}px`,
          left: '50%',
          transform: 'translateX(-50%)',
          width: `calc(100vw - ${VIEWPORT_PADDING * 2}px)`,
          maxWidth: `${POPOVER_WIDTH}px`,
          zIndex: 9999,
        },
        placement: 'bottom',
      };
    }

    if (!anchorPosition) {
      return {
        positionStyle: {
          position: 'fixed',
          top: `${Math.round((winHeight - POPOVER_HEIGHT) / 2)}px`,
          left: `${Math.round((winWidth - POPOVER_WIDTH) / 2)}px`,
          width: `${POPOVER_WIDTH}px`,
          zIndex: 9999,
        },
        placement: 'bottom',
      };
    }

    let top = 0;
    let left = 0;
    let currentPlacement = 'bottom';

    if ('bottom' in anchorPosition && typeof anchorPosition.bottom === 'number') {
      const spaceBelow = winHeight - (anchorPosition.bottom + GAP);
      const spaceAbove = (anchorPosition.top || 0) - GAP;

      if (spaceBelow >= POPOVER_HEIGHT + VIEWPORT_PADDING || spaceBelow >= spaceAbove) {
        top = anchorPosition.bottom + GAP;
        currentPlacement = 'bottom';
      } else {
        top = Math.max(VIEWPORT_PADDING, (anchorPosition.top || 0) - POPOVER_HEIGHT - GAP);
        currentPlacement = 'top';
      }

      const cardLeft = typeof anchorPosition.left === 'number' ? anchorPosition.left : 0;
      left = cardLeft + 8;
    } else {
      const anchorY = typeof anchorPosition.y === 'number' ? anchorPosition.y : 100;
      const spaceBelow = winHeight - (anchorY + GAP);
      const spaceAbove = anchorY - GAP;

      if (spaceBelow >= POPOVER_HEIGHT + VIEWPORT_PADDING || spaceBelow >= spaceAbove) {
        top = anchorY + GAP;
        currentPlacement = 'bottom';
      } else {
        top = Math.max(VIEWPORT_PADDING, anchorY - POPOVER_HEIGHT - GAP);
        currentPlacement = 'top';
      }

      const anchorX = typeof anchorPosition.x === 'number' ? anchorPosition.x : 100;
      left = anchorX - 24;
    }

    left = Math.max(VIEWPORT_PADDING, Math.min(left, winWidth - POPOVER_WIDTH - VIEWPORT_PADDING));

    return {
      positionStyle: {
        position: 'fixed',
        top: `${top}px`,
        left: `${left}px`,
        width: `${POPOVER_WIDTH}px`,
        zIndex: 9999,
      },
      placement: currentPlacement,
    };
  }, [anchorPosition, isMiniMode]);

  if (typeof document === 'undefined') return null;

  return createPortal(
    <div
      className="quick-note-popover-portal"
      onPointerDown={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
      onClick={(e) => e.stopPropagation()}
    >
      <div
        ref={popoverRef}
        className={`quick-note-positioner placement-${placement}`}
        style={positionStyle}
      >
        <div
          className="quick-note-card"
          role="dialog"
          aria-label={t('item.editNote')}
        >
          <div className="quick-note-header">
            <div className="quick-note-title">
              <Pencil size={13} className="quick-note-icon" />
              <span>{t('item.editNote')}</span>
            </div>
            <button
              type="button"
              className="quick-note-close-btn"
              onClick={onClose}
              aria-label={t('common.cancel')}
              title={t('common.cancel')}
            >
              <X size={13} />
            </button>
          </div>

          <div className="quick-note-input-row">
            <input
              ref={inputRef}
              type="text"
              className="quick-note-input"
              value={noteInput}
              onChange={(e) => setNoteInput(e.target.value)}
              onKeyDown={handleKeyDown}
              placeholder={t('edit.notePlaceholder')}
              maxLength={60}
              disabled={busy}
              autoComplete="off"
              spellCheck={false}
            />
            {noteInput && (
              <button
                type="button"
                className="quick-note-clear-btn"
                onClick={() => setNoteInput('')}
                title={t('common.clear')}
                aria-label={t('common.clear')}
              >
                <X size={11} />
              </button>
            )}
            <button
              type="button"
              className="quick-note-confirm-btn"
              onClick={() => void handleSaveAndClose()}
              disabled={busy}
              title={`${t('common.save')} (Enter)`}
              aria-label={`${t('common.save')} (Enter)`}
            >
              <Check size={13} />
            </button>
          </div>

          <div className="quick-note-footer">
            <span className="quick-note-shortcut-hint">
              Enter {t('common.save')} · Esc {t('common.cancel')}
            </span>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}

export default memo(QuickNotePopover);

