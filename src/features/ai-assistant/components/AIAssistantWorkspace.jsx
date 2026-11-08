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
  Code2,
  Trash2,
  SlidersHorizontal,
  RefreshCw,
  CopyPlus,
  Layers,
  Bot,
} from 'lucide-react';
import {
  getAiActions,
  addAiAction,
  updateAiAction,
  deleteAiAction,
  resetAiAction,
  resetAllAiActions,
  detectContentType,
} from '../utils/aiActionStorage';
import { applyAccentColor } from '../../../hooks/useTheme';
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

  // 根据源内容自动侦测情境（'code_log' | 'text'）
  const detectedInitialMode = useMemo(
    () => detectContentType(sourceText, sourceType),
    [sourceText, sourceType]
  );
  const [scenarioMode, setScenarioMode] = useState(detectedInitialMode);

  // 动作列表与激活项（初始不默认选中高亮任何动作，保持纯净常态）
  const [actions, setActions] = useState(() =>
    getAiActions({ mode: detectedInitialMode, sourceText, sourceType })
  );
  const [activeActionId, setActiveActionId] = useState(null);

  // 执行状态与结果
  const [resultText, setResultText] = useState('');
  const [running, setRunning] = useState(false);
  const [errorText, setErrorText] = useState('');
  const [copied, setCopied] = useState(false);
  const [replacedSuccess, setReplacedSuccess] = useState(false);
  const [savedNewSuccess, setSavedNewSuccess] = useState(false);
  const [showSource, setShowSource] = useState(false);
  const [responseMeta, setResponseMeta] = useState({ model: '' });

  // 动作编辑 / 新增弹层状态
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

  // 挂载时刷新主题色兜底，杜绝默认蓝闪烁
  useEffect(() => {
    applyAccentColor();
  }, []);

  // 当外部 item 变化且模式未手动干预时，同步情境感知
  useEffect(() => {
    const nextMode = detectContentType(sourceText, sourceType);
    setScenarioMode(nextMode);
    const list = getAiActions({ mode: nextMode, sourceText, sourceType });
    setActions(list);
    setActiveActionId(null);
    setResultText('');
    setErrorText('');
  }, [item?.id, sourceText, sourceType]);

  // 刷新动作列表
  const reloadActions = useCallback(
    (mode = scenarioMode) => {
      const list = getAiActions({ mode, sourceText, sourceType });
      setActions(list);
      return list;
    },
    [scenarioMode, sourceText, sourceType]
  );

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
  const activeAction = useMemo(
    () => actions.find((a) => a.id === activeActionId) || actions[0] || null,
    [actions, activeActionId]
  );

  // 执行指定动作
  const handleExecuteAction = useCallback(
    async (action) => {
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
        const msg =
          typeof error === 'string'
            ? error
            : error?.message || t('aiAssistant.errorUnknown');
        setErrorText(msg);
      } finally {
        setRunning(false);
      }
    },
    [sourceText, running, t]
  );

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

  // 另存为新项
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
    e?.preventDefault();
    e?.stopPropagation();
    setEditingTarget(action);
    setFormName(action.name);
    setFormPrompt(action.prompt);
    setFormError('');
    setShowManageMenu(false);
    setEditModalOpen(true);
  };

  // 判断表单是否被修改（防误触关闭）
  const isFormDirty = useMemo(() => {
    const originalName = editingTarget ? editingTarget.name : '';
    const originalPrompt = editingTarget ? editingTarget.prompt : '';
    return formName !== originalName || formPrompt !== originalPrompt;
  }, [editingTarget, formName, formPrompt]);

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

  // 恢复预设动作默认提示词
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
    const defaultList = resetAllAiActions(scenarioMode);
    setActions(defaultList);
    setActiveActionId(defaultList[0]?.id || null);
    setShowManageMenu(false);
    onFeedback?.(t('aiAssistant.resetAllActions'));
  };

  // 手动切换情境模式（代码排错 vs 通用文本）
  const handleToggleScenarioMode = () => {
    const nextMode = scenarioMode === 'code_log' ? 'text' : 'code_log';
    setScenarioMode(nextMode);
    const list = getAiActions({ mode: nextMode, sourceText, sourceType });
    setActions(list);
    setActiveActionId(list[0]?.id || null);
    setShowManageMenu(false);
    onFeedback?.(
      nextMode === 'code_log'
        ? t('aiAssistant.scenarioSwitchedCode', '已切换为代码/排错动作')
        : t('aiAssistant.scenarioSwitchedText', '已切换为通用文本动作')
    );
  };

  // 点击遮罩关闭弹层（防误触：修改过内容则必须显式点击取消或保存）
  const handleOverlayClick = (e) => {
    if (e.target !== e.currentTarget) return;
    if (isFormDirty) return; // 脏数据拦截，不准误触关闭丢失
    setEditModalOpen(false);
  };

  return (
    <div
      className="ai-workspace-root"
      onClick={(e) => e.stopPropagation()}
      onMouseDown={(e) => e.stopPropagation()}
    >
      {/* 顶部专属导航条 */}
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
          <div className="ai-workspace-title-area">
            <Bot size={14} className="ai-workspace-title-icon" />
            <span className="ai-workspace-title-text">{t('aiAssistant.title')}</span>
          </div>

          {/* 情境感知微标 */}
          <div
            className={`ai-workspace-scenario-badge ${scenarioMode === 'code_log' ? 'code-mode' : ''}`}
            title={
              scenarioMode === 'code_log'
                ? t('aiAssistant.scenarioBadgeCodeTooltip', '情境感知：已适配代码与日志诊断预设')
                : t('aiAssistant.scenarioBadgeTextTooltip', '情境感知：已适配通用文本预设')
            }
          >
            {scenarioMode === 'code_log' ? <Code2 size={11} /> : <FileText size={11} />}
            <span>
              {scenarioMode === 'code_log'
                ? t('aiAssistant.scenario.codeLog', '代码/排错')
                : t('aiAssistant.scenario.text', '通用文本')}
            </span>
          </div>

          <span className="ai-workspace-meta-tag">
            {t(`aiAssistant.sourceType.${sourceType}`, { defaultValue: sourceType })} ·{' '}
            {sourceText.length} 字
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

      {/* 单行横向固定轨道 */}
      <div className="ai-workspace-track-bar">
        <div className="ai-workspace-track" ref={trackRef}>
          {actions.map((action) => {
            const isActive = action.id === activeActionId;
            return (
              <div
                key={action.id}
                className={`ai-workspace-chip ${isActive ? 'active' : ''} ${
                  running && isActive ? 'running' : ''
                }`}
                onClick={() => handleExecuteAction(action)}
                onContextMenu={(e) => handleOpenEditModal(action, e)}
                title={t('aiAssistant.chipTooltip', {
                  name: action.name,
                  defaultValue: `${action.name} (单击执行 · 右键编辑)`,
                })}
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
                  aria-label={t('aiAssistant.editAction')}
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
              aria-expanded={showManageMenu}
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
                  className="ai-workspace-dropdown-item"
                  onClick={handleToggleScenarioMode}
                >
                  <Layers size={13} />
                  <span>
                    {scenarioMode === 'code_log'
                      ? t('aiAssistant.switchToGeneral', '切换为通用文本预设')
                      : t('aiAssistant.switchToCodeLog', '切换为代码/排错预设')}
                  </span>
                </button>

                <div className="ai-workspace-dropdown-divider" />

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
                <span>{t('common.retry', '重试')}</span>
              </button>
            )}
          </div>
        )}

        {!running && !errorText && !resultText && (
          <div className="ai-workspace-empty">
            <div className="ai-workspace-empty-icon">
              <Bot size={22} />
            </div>
            <div className="ai-workspace-empty-title">{t('aiAssistant.emptyHintTitle')}</div>
            <div className="ai-workspace-empty-sub">{t('aiAssistant.emptyHintSubtitle')}</div>
          </div>
        )}

        {!running && resultText && (
          <div className="ai-workspace-result-body">
            <div className="ai-workspace-result-content">{resultText}</div>
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
            <span>
              {savedNewSuccess ? t('aiAssistant.saveAsNewSuccess') : t('aiAssistant.saveAsNew')}
            </span>
          </button>

          <button
            type="button"
            className={`ai-workspace-btn primary ${replacedSuccess ? 'success' : ''}`}
            onClick={handleReplaceOriginal}
            disabled={!resultText.trim() || running}
            title={t('aiAssistant.replaceOriginal')}
          >
            {replacedSuccess ? <Check size={13} /> : <Wand2 size={13} />}
            <span>
              {replacedSuccess ? t('aiAssistant.replaceSuccess') : t('aiAssistant.replaceOriginal')}
            </span>
          </button>
        </div>
      </footer>

      {/* 动作编辑 / 新增轻量弹层（防误触穿透） */}
      {editModalOpen && (
        <div className="ai-workspace-modal-overlay" onClick={handleOverlayClick}>
          <div className="ai-workspace-modal-card" onClick={(e) => e.stopPropagation()}>
            <div className="ai-workspace-modal-head">
              <span className="ai-workspace-modal-title">
                {editingTarget ? t('aiAssistant.editAction') : t('aiAssistant.addAction')}
              </span>
              <button
                type="button"
                className="ai-workspace-modal-close"
                onClick={() => setEditModalOpen(false)}
                title={t('common.close', '关闭')}
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

              {formError && <div className="ai-workspace-modal-error">{formError}</div>}
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
