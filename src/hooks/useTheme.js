import { useState, useEffect, useCallback } from 'react';

const ACCENT_PRESETS = ['blue', 'purple', 'pink', 'red', 'orange', 'yellow', 'green', 'graphite'];

/**
 * Theme + accent color management hook
 * Supports light / dark / system modes and accent color presets
 */
export function useTheme(initialMode = 'system') {
  const [themeMode, setThemeMode] = useState(initialMode);

  const getEffectiveTheme = useCallback((mode) => {
    if (mode === 'system') {
      return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light';
    }
    return mode;
  }, []);

  const applyTheme = useCallback((theme) => {
    if (theme === 'dark') {
      document.documentElement.classList.add('dark-mode');
    } else {
      document.documentElement.classList.remove('dark-mode');
    }
  }, []);

  useEffect(() => {
    applyTheme(getEffectiveTheme(themeMode));
  }, [themeMode, applyTheme, getEffectiveTheme]);

  useEffect(() => {
    const mediaQuery = window.matchMedia('(prefers-color-scheme: dark)');
    const handler = () => {
      if (themeMode === 'system') {
        applyTheme(mediaQuery.matches ? 'dark' : 'light');
      }
    };
    mediaQuery.addEventListener('change', handler);
    return () => mediaQuery.removeEventListener('change', handler);
  }, [themeMode, applyTheme]);

  return { themeMode, setThemeMode };
}

export const ACCENT_STORAGE_KEY = 'qollect_accent_color';

export function getStoredAccentColor() {
  if (typeof window === 'undefined' || !window.localStorage) return 'blue';
  try {
    return window.localStorage.getItem(ACCENT_STORAGE_KEY) || 'blue';
  } catch {
    return 'blue';
  }
}

export function applyAccentColor(color) {
  const effectiveColor = color || getStoredAccentColor() || 'blue';
  const el = document.documentElement;
  ACCENT_PRESETS.forEach((c) => el.classList.remove(`accent-${c}`));
  if (effectiveColor && effectiveColor !== 'blue') {
    el.classList.add(`accent-${effectiveColor}`);
  }
  try {
    if (typeof window !== 'undefined' && window.localStorage && effectiveColor) {
      window.localStorage.setItem(ACCENT_STORAGE_KEY, effectiveColor);
    }
  } catch {}
}

// 模块加载时立即应用缓存的强调色，杜绝启动时闪烁默认蓝
if (typeof window !== 'undefined' && typeof document !== 'undefined') {
  try {
    const cached = window.localStorage.getItem(ACCENT_STORAGE_KEY);
    if (cached && cached !== 'blue' && ACCENT_PRESETS.includes(cached)) {
      document.documentElement.classList.add(`accent-${cached}`);
    }
  } catch {}
}
