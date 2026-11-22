export const PREVIEW_CLICK_DELAY_MS = 320;
export const SORT_MODE_LONG_PRESS_MS = 190;
export const SORT_MODE_LONG_PRESS_MOVE_PX = 10;

// True interactive controls inside a card that must never trigger card-level
// double-click copy/paste, drag-start, or long-press sort.
export const INTERACTIVE_SUB_ELEMENT_SELECTOR = [
  'button',
  'input',
  'textarea',
  'select',
  'a',
  '[role="button"]',
  '[role="menu"]',
  '[role="menuitem"]',
  '[role="dialog"]',
  '[contenteditable="true"]',
  '[data-longpress-ignore="true"]',
  '.color-picker-popover',
  '.panel-card-action-btn',
  '.compact-card-action-btn',
  '.compact-card-select-toggle',
  '.compact-card-alias-header',
  '.compact-tag.clickable',
].join(', ');

// Selectable text regions inside cards: dragging/long-pressing here should select text
// rather than start card drag/sort, but double-clicking MUST still trigger card copy/auto-paste.
export const TEXT_SELECTION_SUB_ELEMENT_SELECTOR = [
  '.compact-card-text',
  '.compact-card-note',
].join(', ');

const DRAG_AND_LONG_PRESS_IGNORE_SELECTOR = [
  INTERACTIVE_SUB_ELEMENT_SELECTOR,
  TEXT_SELECTION_SUB_ELEMENT_SELECTOR,
].join(', ');

export const shouldIgnoreLongPressTarget = (target) => {
  if (typeof Element === 'undefined') return false;
  return target instanceof Element && Boolean(target.closest(DRAG_AND_LONG_PRESS_IGNORE_SELECTOR));
};

export const shouldIgnoreDragStartTarget = (target) => {
  if (typeof Element === 'undefined') return false;
  return target instanceof Element && Boolean(target.closest(DRAG_AND_LONG_PRESS_IGNORE_SELECTOR));
};

export const shouldIgnoreCardDoubleClickTarget = (target, currentTarget = null) => {
  if (typeof Element === 'undefined') return false;
  if (!(target instanceof Element)) return false;
  const ignoredTarget = target.closest(INTERACTIVE_SUB_ELEMENT_SELECTOR);
  if (!ignoredTarget) return false;
  if (currentTarget instanceof Element && ignoredTarget === currentTarget) {
    return false;
  }
  return true;
};

export const openItemPreview = (setPanelMode, item, options) => {
  if (typeof setPanelMode !== 'function' || !item) return;
  setPanelMode('preview', item, options);
};
