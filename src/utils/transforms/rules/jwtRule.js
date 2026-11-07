import { ShieldCheck } from 'lucide-react';
import { TRANSFORM_CATEGORIES } from '../types.js';

/**
 * 诊断并解码 JWT Token
 * @param {string} text
 * @returns {string|null}
 */
export function safeJwtDecode(text) {
  if (typeof text !== 'string') return null;
  const trimmed = text.trim();
  const parts = trimmed.split('.');
  if (parts.length !== 3) return null;
  if (!/^[A-Za-z0-9_-]+$/.test(parts[0]) || !/^[A-Za-z0-9_-]+$/.test(parts[1]) || !/^[A-Za-z0-9_-]+$/.test(parts[2])) {
    return null;
  }

  const decodeB64Url = (str) => {
    let standard = str.replace(/-/g, '+').replace(/_/g, '/');
    while (standard.length % 4 !== 0) standard += '=';
    if (typeof atob === 'function') {
      const binary = atob(standard);
      const bytes = new Uint8Array(binary.length);
      for (let i = 0; i < binary.length; i++) {
        bytes[i] = binary.charCodeAt(i);
      }
      return new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    }
    return Buffer.from(standard, 'base64').toString('utf8');
  };

  try {
    const headerStr = decodeB64Url(parts[0]);
    const payloadStr = decodeB64Url(parts[1]);
    const header = JSON.parse(headerStr);
    const payload = JSON.parse(payloadStr);

    if (typeof header !== 'object' || typeof payload !== 'object' || header === null || payload === null) {
      return null;
    }

    const formatTimestamp = (ts) => {
      if (typeof ts !== 'number' || ts <= 0) return null;
      const d = new Date(ts > 1e11 ? ts : ts * 1000);
      if (isNaN(d.getTime())) return null;
      return d.toLocaleString('zh-CN', { hour12: false });
    };

    const meta = {};
    const nowSec = Math.floor(Date.now() / 1000);

    if (typeof payload.exp === 'number') {
      const expDate = formatTimestamp(payload.exp);
      const isExpired = payload.exp < nowSec;
      const diffSec = Math.abs(payload.exp - nowSec);
      const diffMin = Math.round(diffSec / 60);
      const diffHours = (diffSec / 3600).toFixed(1);
      meta.expires_at = expDate;
      meta.status = isExpired ? `EXPIRED (已过期 ${diffHours} 小时前)` : `VALID (剩余约 ${diffMin > 120 ? diffHours + '小时' : diffMin + '分钟'})`;
    }

    if (typeof payload.iat === 'number') {
      meta.issued_at = formatTimestamp(payload.iat);
    }

    const result = {
      ...(Object.keys(meta).length > 0 ? { _meta: meta } : {}),
      header,
      payload,
    };
    return JSON.stringify(result, null, 2);
  } catch {
    return null;
  }
}

export const jwtRule = {
  id: 'jwt_decode',
  category: TRANSFORM_CATEGORIES.CONVERT,
  icon: ShieldCheck,
  labelKey: 'transforms.jwtDecode',
  titleKey: 'transforms.jwtDecodeTitle',
  priority: 48,
  previewable: false,

  match: (text) => {
    return safeJwtDecode(text) !== null;
  },

  apply: (text) => {
    const decoded = safeJwtDecode(text);
    return decoded !== null ? decoded : text;
  },
};
