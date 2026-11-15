import { invoke } from '@tauri-apps/api/core';
import { buildItemDerivations } from './itemsIndex';

export const createNoteActions = (set, get) => ({
  startEditNote: (item) => {
    set({ editingNoteId: item.id, editNote: item.user_note || '' });
  },

  saveNote: async (item) => {
    try {
      const { editNote, loadItems } = get();
      const updatedItem = {
        ...item,
        user_note: editNote,
        updated_at: Math.floor(Date.now() / 1000),
      };
      await invoke('update_item', { item: updatedItem });
      await loadItems();
      set((state) => ({
        editingNoteId: null,
        editNote: '',
        previewItem: state.previewItem?.id === item.id ? updatedItem : state.previewItem,
      }));
    } catch (error) {
      console.error('保存备注失败:', error);
    }
  },

  cancelNoteEdit: () => {
    set({ editingNoteId: null, editNote: '' });
  },

  updateItemNote: async (item, note) => {
    try {
      const latestItem = get().items?.find((i) => i.id === item.id) || item;
      const trimmed = typeof note === 'string' ? note.trim() : '';
      const updatedAt = Math.floor(Date.now() / 1000);
      const updatedItem = {
        ...latestItem,
        user_note: trimmed,
        updated_at: updatedAt,
      };
      await invoke('update_item', { item: updatedItem });
      set((state) => {
        const nextItems = (state.items || []).map((candidate) => (
          candidate.id === updatedItem.id
            ? { ...candidate, user_note: updatedItem.user_note, updated_at: updatedAt }
            : candidate
        ));
        return {
          items: nextItems,
          ...buildItemDerivations(nextItems),
          itemsRevision: (state.itemsRevision || 0) + 1,
          previewItem: state.previewItem?.id === updatedItem.id
            ? { ...state.previewItem, user_note: updatedItem.user_note, updated_at: updatedAt }
            : state.previewItem,
        };
      });
      await get().loadItems?.();
    } catch (error) {
      console.error('[noteActions] updateItemNote failed:', error);
      throw error;
    }
  },
});

