// Toasts and the notification centre (the bell).
import type { StateCreator } from "zustand";
import type { Store } from "../../store";
import { NOTICE_LIMIT, noticeVisible } from "../defaults";
import type { Notice, NoticeFilter, Toast } from "../types";

export interface NoticesSlice {
  toasts: Toast[];
  /** Notification center (the bell): failures stay here after their toast fades. Not saved between runs. */
  notices: Notice[];
  noticeFilter: NoticeFilter;
  /** The bell menu is open (a clicked desktop notification opens it). */
  noticeMenuOpen: boolean;
  setNoticeMenuOpen: (open: boolean) => void;
  setNoticeFilter: (f: NoticeFilter) => void;
  markNotice: (id: number, read: boolean) => void;
  /** "Mark all as read / unread": applies to the notifications the current filter shows. */
  markAllNotices: (read: boolean) => void;
  removeNotice: (id: number) => void;
  toast: (ok: boolean, text: string) => void;
  dropToast: (id: number) => void;
}

export const createNoticesSlice: StateCreator<Store, [], [], NoticesSlice> = (set, get) => {
  return {
    toasts: [],
    notices: [],
    noticeFilter: "All",
    noticeMenuOpen: false,
    setNoticeMenuOpen: (noticeMenuOpen) => set({ noticeMenuOpen }),
    setNoticeFilter: (noticeFilter) => set({ noticeFilter }),
    markNotice: (id, read) => set((s) => ({ notices: s.notices.map((n) => (n.id === id ? { ...n, readAt: read ? Date.now() : null } : n)) })),
    markAllNotices: (read) =>
      set((s) => ({ notices: s.notices.map((n) => (noticeVisible(n, s.noticeFilter) ? { ...n, readAt: read ? Date.now() : null } : n)) })),
    removeNotice: (id) => set((s) => ({ notices: s.notices.filter((n) => n.id !== id) })),
    toast: (ok, text) => {
      const id = Date.now() + Math.random();
      set((s) => {
        const notices = ok ? s.notices : [...s.notices.slice(-(NOTICE_LIMIT - 1)), { id, text, at: Date.now(), readAt: null }];
        return { toasts: [...s.toasts.slice(-3), { id, ok, text }], notices };
      });
      setTimeout(() => get().dropToast(id), ok ? 3500 : 7000);
    },
    dropToast: (id) => set((s) => ({ toasts: s.toasts.filter((t) => t.id !== id) })),
  };
};
