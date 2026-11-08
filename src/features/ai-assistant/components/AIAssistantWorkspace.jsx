import { useState, useEffect, useMemo, useRef, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { invoke } from '@tauri-apps/api/core';
import {
  ArrowLeft,
  X,
  Wand2,
  Plus,
  Pencil,
  RotateCcw,
  Copy,
  Check,
  Loader2,
  ChevronDown,
  ChevronUp,
  Sparkles,
  FileText,
  Trash2,
  SlidersHorizontal,
  RefreshCw,
  CopyPlus
} from 'lucide-react';
import {
  getAiActions,
  addAiAction,
  updateAiAction,
  deleteAiAction,
  resetAiAction,
  resetAllAiActions,
} from '../utils/aiActionStorage';
import { runCustomAiAction } from '../services/promptAssistantService';
import './AIAssistantWorkspace.css';

function AIAssistantWorkspace({
  item,
  sourceText = '',
  sourceType = 'text',
  onClose,
  onItemUpdated,
  onFeedback,
}) {
  const { t } = useTranslation();
  const [actions, setActions] = useState(() => getAiActions());
  const [activeActionId, setActiveActionId] = useState(() => actions[0]?.id || null);
  const [resultText, setResultText] = useState('');
  const [running, setRunning] = useState(false);
  const [errorText, setErrorText] = useState('');
  const [copied, setCopied] = useState(false);
  const [replacedSuccess, setReplacedSuccess] = useState(false);
  const [savedNewSuccess, setSavedNewSuccess] = useState(false);
  const [showSource, setShowSource] = useState(false);
  const [responseMeta, setResponseMeta] = useState({ model: '' });

  // 动作编辑/新增弹层状态
  const [editModalOpen, setEditModalOpen] = useState(false);
  const [editingTarget, setEditingTarget] = useState(null); // null 表示新建
  const [formName, setFormName] = useState('');
  const [formPrompt, setFormPrompt] = useState('');
  const [formError, setFormError] = useState('');

  // 更多管理下拉
  const [showManageMenu, setShowManageMenu] = useState(false);

  const trackRef = useRef(null);
  const resultCardRef = useRef(null);
  const manageMenuRef = useRef(null);

  // 刷新本地动作列表
  const reloadActions = useCallback(() => {
    const list = getAiActions();
    setActions(list);
    return list;
  }, []);

  // 复制反馈计时器
  useEffect(() => {
    if (!copied) return undefined;
    const timer = setTimeout(() => setCopied(false), 1400);
    return () => clearTimeout(timer);
  }, [copied]);

  // 替换成功反馈计时器
  useEffect(() => {
    if (!replacedSuccess) return undefined;
    const timer = setTimeout(() => setReplacedSuccess(false), 1400);
    return () => clearTimeout(timer);
  }, [replacedSuccess]);

  // 另存为新项反馈计时器
  useEffect(() => {
    if (!savedNewSuccess) return undefined;
    const timer = setTimeout(() => setSavedNewSuccess(false), 1400);
    return () => clearTimeout(timer);
  }, [savedNewSuccess]);

  // 点击外部关闭管理下拉
  useEffect(() => {
    if (!showManageMenu) return undefined;
    const handleOutsideClick = (e) => {
      if (manageMenuRef.current && !manageMenuRef.current.contains(e.target)) {
        setShowManageMenu(false);
      }
    };
    document.addEventListener('mousedown', handleOutsideClick);
    return () => document.removeEventListener('mousedown', handleOutsideClick);
  }, [showManageMenu]);

  // 当前激活的动作对象
  const activeAction = useMemo(() => (
    actions.find((a) => a.id === activeActionId) || actions[0] || null
  ), [actions, activeActionId]);

  // 执行某个指定动作
  const handleExecuteAction = useCallback(async (action) => {
    if (!sourceText.trim()) {
      setErrorText(t('aiAssistant.noInput'));
      return;
    }
    if (running) return;

    setActiveActionId(action.id);
    setRunning(true);
    setErrorText('');
    setResultText('');

    try {
      const response = await runCustomAiAction({
        content: sourceText.trim(),
        prompt: action.prompt.trim(),
      });

      setResultText(response?.output || '');
      setResponseMeta({
        model: response?.model || '',
      });

      requestAnimationFrame(() => {
        resultCardRef.current?.scrollIntoView({
          behavior: 'smooth',
          block: 'nearest',
        });
      });
    } catch (error) {
      const msg = typeof error === 'string' ? error : error?.message || t('aiAssistant.errorUnknown');
      setErrorText(msg);
    } finally {
      setRunning(false);
    }
  }, [sourceText, running, t]);

  // 复制结果
  const handleCopyResult = async () => {
    if (!resultText.trim()) return;
    try {
      await invoke('copy_plain_text', { text: resultText });
      setCopied(true);
      onFeedback?.(t('aiAssistant.copied'));
    } catch (err) {
      setErrorText(String(err));
    }
  };

  // 替换原卡片内容
  const handleReplaceOriginal = async () => {
    if (!resultText.trim() || !item?.id) return;
    try {
      const updatedAt = Math.floor(Date.now() / 1000);
      const updatedItem = {
        ...item,
        content: resultText,
        content_updated: true,
        content_truncated: false,
        updated_at: updatedAt,
      };

      await invoke('update_item', { item: updatedItem });
      onItemUpdated?.(updatedItem);
      setReplacedSuccess(true);
      onFeedback?.(t('aiAssistant.replaceSuccess'));
    } catch (err) {
      setErrorText(String(err));
    }
  };

  // 另存为新项（写入剪贴板由系统监听器自动作为新卡片抓取）
  const handleSaveAsNew = async () => {
    if (!resultText.trim()) return;
    try {
      await invoke('copy_plain_text', { text: resultText });
      setSavedNewSuccess(true);
      onFeedback?.(t('aiAssistant.saveAsNewSuccess'));
    } catch (err) {
      setErrorText(String(err));
    }
  };

  // 打开新建弹层
  const handleOpenAddModal = () => {
    setEditingTarget(null);
    setFormName('');
    setFormPrompt('');
    setFormError('');
    setShowManageMenu(false);
    setEditModalOpen(true);
  };

  // 打开编辑弹层
  const handleOpenEditModal = (action, e) => {
    e?.stopPropagation();
    setEditingTarget(action);
    setFormName(action.name);
    setFormPrompt(action.prompt);
    setFormError('');
    setShowManageMenu(false);
    setEditModalOpen(true);
  };

  // 保存动作（新增或编辑）
  const handleSaveActionForm = () => {
    const trimmedName = formName.trim();
    const trimmedPrompt = formPrompt.trim();

    if (!trimmedName) {
      setFormError(t('aiAssistant.actionNamePlaceholder'));
      return;
    }
    if (!trimmedPrompt) {
      setFormError(t('aiAssistant.actionPromptPlaceholder'));
      return;
    }

    try {
      if (editingTarget) {
        updateAiAction(editingTarget.id, {
          name: trimmedName,
          prompt: trimmedPrompt,
        });
      } else {
        const added = addAiAction({
          name: trimmedName,
          prompt: trimmedPrompt,
        });
        setActiveActionId(added.id);
      }
      reloadActions();
      setEditModalOpen(false);
    } catch (err) {
      setFormError(String(err?.message || err));
    }
  };

  // 删除当前编辑动作
  const handleDeleteEditingAction = () => {
    if (!editingTarget || editingTarget.isPreset) return;
    try {
      deleteAiAction(editingTarget.id);
      const nextList = reloadActions();
      if (activeActionId === editingTarget.id) {
        setActiveActionId(nextList[0]?.id || null);
      }
      setEditModalOpen(false);
    } catch (err) {
      setFormError(String(err?.message || err));
    }
  };

  // 恢复预设动作
  const handleResetEditingAction = () => {
    if (!editingTarget || !editingTarget.isPreset) return;
    try {
      resetAiAction(editingTarget.id);
      reloadActions();
      setEditModalOpen(false);
    } catch (err) {
      setFormError(String(err?.message || err));
    }
  };

  // 恢复全部默认预设
  const handleResetAll = () => {
    const defaultList = resetAllAiActions();
    setActions(defaultList);
    setActiveActionId(defaultList[0]?.id || null);
    setShowManageMenu(false);
    onFeedback?.(t('aiAssistant.resetAllActions'));
  };

  return (
    <div className="ai-workspace-root">
      {/* 顶部专属导航条（完全覆盖原 preview-header） */}
      <header className="ai-workspace-header">
        <div className="ai-workspace-header-left">
          <button
            type="button"
            className="ai-workspace-back-btn"
            onClick={onClose}
            title={t('aiAssistant.back')}
          >
            <ArrowLeft size={15} />
            <span className="ai-workspace-back-label">{t('aiAssistant.back')}</span>
          </button>
          <div className="ai-workspace-title-pill">
            <Sparkles size={13} className="ai-workspace-sparkle" />
            <span className="ai-workspace-title-text">{t('aiAssistant.title')}</span>
          </div>
          <span className="ai-workspace-meta-tag">
            {t(`aiAssistant.sourceType.${sourceType}`, { defaultValue: sourceType })} · {sourceText.length} 字
          </span>
        </div>

        <div className="ai-workspace-header-right">
          <button
            type="button"
            className="ai-workspace-close-btn"
            onClick={onClose}
            title={t('aiAssistant.close')}
          >
            <X size={15} />
          </button>
        </div>
      </header>

      {/* 单行横向固定轨道（锁定 40px 高度，严禁换行） */}
      <div className="ai-workspace-track-bar">
        <div className="ai-workspace-track" ref={trackRef}>
          {actions.map((action) => {
            const isActive = action.id === activeActionId;
            return (
              <div
                key={action.id}
                className={`ai-workspace-chip ${isActive ? 'active' : ''} ${running && isActive ? 'running' : ''}`}
                onClick={() => handleExecuteAction(action)}
                role="button"
                tabIndex={0}
                onKeyDown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    e.preventDefault();
                    handleExecuteAction(action);
                  }
                }}
              >
                <span className="ai-workspace-chip-name">{action.name}</span>
                <button
                  type="button"
                  className="ai-workspace-chip-edit-btn"
                  onClick={(e) => handleOpenEditModal(action, e)}
                  title={t('aiAssistant.editAction')}
                >
                  <Pencil size={11} />
                </button>
              </div>
            );
          })}
        </div>

        {/* 轨道右侧固定操作区：新增动作与更多管理 */}
        <div className="ai-workspace-track-controls">
          <button
            type="button"
            className="ai-workspace-track-icon-btn"
            onClick={handleOpenAddModal}
            title={t('aiAssistant.addAction')}
          >
            <Plus size={14} />
          </button>

          <div className="ai-workspace-more-wrap" ref={manageMenuRef}>
            <button
              type="button"
              className={`ai-workspace-track-icon-btn ${showManageMenu ? 'active' : ''}`}
              onClick={() => setShowManageMenu((prev) => !prev)}
              title={t('aiAssistant.manageActions')}
            >
              <SlidersHorizontal size={13} />
            </button>

            {showManageMenu && (
              <div className="ai-workspace-dropdown-menu">
                <button
                  type="button"
                  className="ai-workspace-dropdown-item"
                  onClick={handleOpenAddModal}
                >
                  <Plus size={13} />
                  <span>{t('aiAssistant.addAction')}</span>
                </button>
                <button
                  type="button"
                  className="ai-workspace-dropdown-item danger"
                  onClick={handleResetAll}
                >
                  <RotateCcw size={13} />
                  <span>{t('aiAssistant.resetAllActions')}</span>
                </button>
              </div>
            )}
          </div>
        </div>
      </div>

      {/* 原文可折叠对照条 */}
      <div className="ai-workspace-source-collapse">
        <button
          type="button"
          className="ai-workspace-source-toggle"
          onClick={() => setShowSource((prev) => !prev)}
        >
          <FileText size={12} />
          <span className="ai-workspace-source-summary">
            {sourceText.slice(0, 48).replace(/\n/g, ' ')}...
          </span>
          <span className="ai-workspace-source-icon">
            {showSource ? <ChevronUp size={13} /> : <ChevronDown size={13} />}
          </span>
        </button>
        {showSource && (
          <div className="ai-workspace-source-content">
            <pre>{sourceText}</pre>
          </div>
        )}
      </div>

      {/* 结果呈现画布（100% 弹性填满剩余高度） */}
      <div className="ai-workspace-canvas" ref={resultCardRef}>
        {running && (
          <div className="ai-workspace-status running">
            <Loader2 size={20} className="ai-spin" />
            <span className="ai-workspace-status-text">
              {t('aiAssistant.runningAction', { name: activeAction?.name || '' })}
            </span>
          </div>
        )}

        {errorText && !running && (
          <div className="ai-workspace-status error">
            <div className="ai-workspace-error-msg">{errorText}</div>
            {activeAction && (
              <button
                type="button"
                className="ai-workspace-retry-btn"
                onClick={() => handleExecuteAction(activeAction)}
              >
                <RefreshCw size={12} />
                <span>重试</span>
              </button>
            )}
          </div>
        )}

        {!running && !errorText && !resultText && (
          <div className="ai-workspace-empty">
            <div className="ai-workspace-empty-icon">
              <Wand2 size={26} />
            </div>
            <div className="ai-workspace-empty-title">{t('aiAssistant.emptyHintTitle')}</div>
            <div className="ai-workspace-empty-sub">{t('aiAssistant.emptyHintSubtitle')}</div>
          </div>
        )}

        {!running && resultText && (
          <div className="ai-workspace-result-body">
            <div className="ai-workspace-result-content">
              {resultText}
            </div>
          </div>
        )}
      </div>

      {/* 底部操作工具栏 */}
      <footer className="ai-workspace-footer">
        <div className="ai-workspace-footer-meta">
          {responseMeta.model && (
            <span className="ai-workspace-meta-model">{responseMeta.model}</span>
          )}
          {resultText && (
            <span className="ai-workspace-meta-chars">
              {t('aiAssistant.chars', { count: resultText.length })}
            </span>
          )}
        </div>

        <div className="ai-workspace-footer-actions">
          <button
            type="button"
            className={`ai-workspace-btn secondary ${copied ? 'success' : ''}`}
            onClick={handleCopyResult}
            disabled={!resultText.trim() || running}
          >
            {copied ? <Check size={13} /> : <Copy size={13} />}
            <span>{copied ? t('aiAssistant.copied') : t('aiAssistant.copy')}</span>
          </button>

          <button
            type="button"
            className={`ai-workspace-btn secondary ${savedNewSuccess ? 'success' : ''}`}
            onClick={handleSaveAsNew}
            disabled={!resultText.trim() || running}
            title={t('aiAssistant.saveAsNew')}
          >
            {savedNewSuccess ? <Check size={13} /> : <CopyPlus size={13} />}
            <span>{savedNewSuccess ? t('aiAssistant.saveAsNewSuccess') : t('aiAssistant.saveAsNew')}</span>
          </button>

          <button
            type="button"
            className={`ai-workspace-btn primary ${replacedSuccess ? 'success' : ''}`}
            onClick={handleReplaceOriginal}
            disabled={!resultText.trim() || running}
            title={t('aiAssistant.replaceOriginal')}
          >
            {replacedSuccess ? <Check size={13} /> : <Wand2 size={13} />}
            <span>{replacedSuccess ? t('aiAssistant.replaceSuccess') : t('aiAssistant.replaceOriginal')}</span>
          </button>
        </div>
      </footer>

      {/* 动作编辑 / 新增轻量弹层 */}
      {editModalOpen && (
        <div
          className="ai-workspace-modal-overlay"
          onClick={() => setEditModalOpen(false)}
        >
          <div
            className="ai-workspace-modal-card"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="ai-workspace-modal-head">
              <span className="ai-workspace-modal-title">
                {editingTarget ? t('aiAssistant.editAction') : t('aiAssistant.addAction')}
              </span>
              <button
                type="button"
                className="ai-workspace-modal-close"
                onClick={() => setEditModalOpen(false)}
              >
                <X size={14} />
              </button>
            </div>

            <div className="ai-workspace-modal-body">
              <label className="ai-workspace-modal-field">
                <span className="ai-workspace-modal-label">{t('aiAssistant.actionName')}</span>
                <input
                  type="text"
                  className="ai-workspace-modal-input"
                  value={formName}
                  maxLength={12}
                  onChange={(e) => setFormName(e.target.value)}
                  placeholder={t('aiAssistant.actionNamePlaceholder')}
                />
              </label>

              <label className="ai-workspace-modal-field">
                <span className="ai-workspace-modal-label">{t('aiAssistant.actionPrompt')}</span>
                <textarea
                  className="ai-workspace-modal-textarea"
                  value={formPrompt}
                  rows={4}
                  onChange={(e) => setFormPrompt(e.target.value)}
                  placeholder={t('aiAssistant.actionPromptPlaceholder')}
                />
              </label>

              {formError && (
                <div className="ai-workspace-modal-error">{formError}</div>
              )}
            </div>

            <div className="ai-workspace-modal-footer">
              <div className="ai-workspace-modal-footer-left">
                {editingTarget?.isPreset && (
                  <button
                    type="button"
                    className="ai-workspace-modal-btn danger-link"
                    onClick={handleResetEditingAction}
                  >
                    <RotateCcw size={12} />
                    <span>{t('aiAssistant.resetAction')}</span>
                  </button>
                )}
                {editingTarget && !editingTarget.isPreset && (
                  <button
                    type="button"
                    className="ai-workspace-modal-btn danger-link"
                    onClick={handleDeleteEditingAction}
                  >
                    <Trash2 size={12} />
                    <span>{t('aiAssistant.deleteAction')}</span>
                  </button>
                )}
              </div>

              <div className="ai-workspace-modal-footer-right">
                <button
                  type="button"
                  className="ai-workspace-modal-btn secondary"
                  onClick={() => setEditModalOpen(false)}
                >
                  {t('common.cancel', '取消')}
                </button>
                <button
                  type="button"
                  className="ai-workspace-modal-btn primary"
                  onClick={handleSaveActionForm}
                >
                  {t('aiAssistant.saveAction')}
                </button>
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

export default AIAssistantWorkspace;
