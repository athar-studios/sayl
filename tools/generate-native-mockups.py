#!/usr/bin/env python3
"""
SAYL Native Desktop UI Mockup Generator (Part 2)
-----------------------------------------------
Generates high-fidelity visual mockups matching native Windows desktop app
conventions (IDM / FDM style) with Sayl brand accents (navy #0b1a4e, turquoise #00b6bd):

1. main-window.png    - Native window with menu bar, large icon toolbar, left category tree, dense grid, status bar (RTL Arabic).
2. main-window-en.png - LTR English toggle view of the native window.
3. context-menu.png   - Right-click native context menu over tasks.
4. delete-dialog.png  - Native-style delete confirmation dialog (IDM style with Recycle Bin checkbox).

Uses headless Google Chrome to render exact HTML/CSS designs to pixel-perfect PNGs.
"""

import os
import subprocess
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]
OUTPUT_DIR = REPO_ROOT / "docs" / "ui-mockups" / "native"
CHROME_PATH = r"C:\Program Files\Google\Chrome\Application\chrome.exe"

COMMON_CSS = """
:root {
  --font-system: 'Segoe UI Variable Text', 'Segoe UI', Tahoma, Arial, sans-serif;
  --font-mono: 'Consolas', 'Cascadia Code', monospace;
  --color-navy: #0b1a4e;
  --color-turquoise: #00b6bd;
  --color-turquoise-dark: #008f95;
  --color-turquoise-light: #e6f8f8;
  --color-accent-blue: #0078d4;
  --color-accent-blue-light: #cce8ff;
  --color-accent-blue-border: #99d1ff;
  --color-bg-window: #f3f3f3;
  --color-bg-card: #ffffff;
  --color-border-grid: #e5e7eb;
  --color-border-subtle: #d1d5db;
  --color-border-dark: #9ca3af;
  --color-text-main: #1f2937;
  --color-text-muted: #6b7280;
  --color-text-dark: #111827;
  --color-state-downloading: #00b6bd;
  --color-state-completed: #16a34a;
  --color-state-paused: #d97706;
  --color-state-failed: #dc2626;
  --color-state-queued: #4b5563;
}

* {
  box-sizing: border-box;
  margin: 0;
  padding: 0;
  user-select: none;
  -webkit-user-select: none;
}

body {
  font-family: var(--font-system);
  font-size: 13px;
  color: var(--color-text-main);
  background: var(--color-bg-window);
  overflow: hidden;
  line-height: 1.35;
}

.window-frame {
  width: 1280px;
  height: 820px;
  display: flex;
  flex-direction: column;
  background: #ffffff;
  border: 1px solid #c5c5c5;
  box-shadow: 0 10px 30px rgba(0,0,0,0.25);
  overflow: hidden;
  position: relative;
}

/* Title bar */
.titlebar {
  height: 34px;
  background: #f9f9fb;
  border-bottom: 1px solid #e5e5e5;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  font-size: 12px;
  font-weight: 500;
  color: #333333;
}

.title-content {
  display: flex;
  align-items: center;
  gap: 8px;
}

.app-icon {
  width: 18px;
  height: 18px;
  border-radius: 3px;
}

.window-caption-buttons {
  display: flex;
  align-items: center;
  gap: 1px;
}

.caption-btn {
  width: 44px;
  height: 32px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #555555;
  font-size: 11px;
  background: transparent;
  border: none;
  cursor: default;
}

.caption-btn.close:hover {
  background: #e81123;
  color: white;
}

/* Classic Menu Bar */
.menubar {
  height: 26px;
  background: #fdfdfd;
  border-bottom: 1px solid #e0e0e0;
  display: flex;
  align-items: center;
  padding: 0 6px;
  gap: 2px;
  font-size: 12px;
}

.menu-item {
  padding: 3px 8px;
  border-radius: 3px;
  cursor: default;
  color: #222222;
}

.menu-item:hover, .menu-item.active {
  background: #e8e8ec;
}

/* Classic Large Icon Toolbar */
.toolbar {
  height: 72px;
  background: #f6f7f9;
  border-bottom: 1px solid #dcdcdc;
  display: flex;
  align-items: center;
  padding: 0 8px;
  gap: 4px;
}

.toolbar-btn {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  width: 72px;
  height: 62px;
  border: 1px solid transparent;
  border-radius: 4px;
  background: transparent;
  cursor: default;
  color: #333333;
  gap: 3px;
  transition: all 0.1s;
}

.toolbar-btn:hover {
  background: #e9ecf1;
  border-color: #d0d7de;
}

.toolbar-btn:active {
  background: #dde2ea;
}

.toolbar-btn.disabled {
  opacity: 0.42;
  cursor: default;
}

.toolbar-btn.disabled:hover {
  background: transparent;
  border-color: transparent;
}

.toolbar-icon {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
}

.toolbar-label {
  font-size: 11px;
  font-weight: 500;
  white-space: nowrap;
}

.toolbar-sep {
  width: 1px;
  height: 48px;
  background: #dcdcdc;
  margin: 0 4px;
}

.toolbar-search {
  margin-inline-start: auto;
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 0 6px;
}

.search-input {
  height: 28px;
  width: 190px;
  border: 1px solid #c5cbd3;
  border-radius: 3px;
  padding: 0 8px;
  font-size: 12px;
  font-family: inherit;
  background: #ffffff;
  outline: none;
}

.search-input:focus {
  border-color: var(--color-turquoise);
}

/* Main Layout */
.main-content {
  flex: 1;
  display: flex;
  overflow: hidden;
  background: #ffffff;
}

/* Left Category Tree */
.sidebar-tree {
  width: 230px;
  border-inline-end: 1px solid #dcdcdc;
  background: #fafafb;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  font-size: 12.5px;
}

.tree-section-header {
  padding: 8px 12px 4px 12px;
  font-size: 11px;
  font-weight: 700;
  text-transform: uppercase;
  color: #6b7280;
  letter-spacing: 0.03em;
}

.tree-node {
  display: flex;
  align-items: center;
  padding: 5px 12px;
  gap: 7px;
  cursor: default;
  color: #2b3340;
  border-inline-start: 3px solid transparent;
}

.tree-node:hover {
  background: #f0f2f5;
}

.tree-node.active {
  background: var(--color-turquoise-light);
  color: var(--color-navy);
  font-weight: 600;
  border-inline-start: 3px solid var(--color-turquoise);
}

.tree-node .node-icon {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
  display: flex;
  align-items: center;
  justify-content: center;
}

.tree-node .node-title {
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.tree-node .node-badge {
  font-size: 11px;
  font-weight: 600;
  color: #6b7280;
  background: #eef0f4;
  padding: 1px 6px;
  border-radius: 10px;
}

.tree-node.active .node-badge {
  background: rgba(0, 182, 189, 0.2);
  color: var(--color-navy);
}

.tree-subnode {
  padding-inline-start: 28px;
}

/* Grid Area */
.grid-container {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: #ffffff;
}

/* Dense Data Grid Table */
.data-grid {
  width: 100%;
  border-collapse: collapse;
  font-size: 12.5px;
  table-layout: fixed;
}

.grid-header th {
  background: #f4f5f7;
  border-bottom: 1px solid #d2d6dc;
  border-inline-end: 1px solid #e1e4e8;
  padding: 5px 8px;
  font-weight: 600;
  color: #374151;
  text-align: start;
  height: 28px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  position: relative;
}

.grid-header th:hover {
  background: #eaecef;
}

.sort-arrow {
  display: inline-block;
  margin-inline-start: 4px;
  font-size: 9px;
  color: #00b6bd;
}

.grid-row {
  height: 26px;
  border-bottom: 1px solid #f0f2f5;
  cursor: default;
}

.grid-row:nth-child(even) {
  background: #fdfdfe;
}

.grid-row:hover {
  background: #f2f7fc;
}

.grid-row.selected {
  background: var(--color-accent-blue-light) !important;
  color: #0b1a4e;
}

.grid-row td {
  padding: 2px 8px;
  border-inline-end: 1px solid #f0f2f5;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  vertical-align: middle;
}

.col-filename {
  display: flex;
  align-items: center;
  gap: 6px;
}

.file-type-icon {
  width: 16px;
  height: 16px;
  flex-shrink: 0;
}

.cell-progress-bar {
  width: 100%;
  height: 15px;
  background: #eef1f5;
  border-radius: 2px;
  position: relative;
  overflow: hidden;
  border: 1px solid #d8dde6;
}

.cell-progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #00b6bd, #06aab9);
  transition: width 0.2s;
}

.cell-progress-fill.completed {
  background: #16a34a;
}

.cell-progress-fill.paused {
  background: #d97706;
}

.cell-progress-text {
  position: absolute;
  inset: 0;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 10.5px;
  font-weight: 700;
  color: #0b1a4e;
  text-shadow: 0 0 2px rgba(255,255,255,0.8);
}

.state-badge {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-weight: 600;
  font-size: 11.5px;
}

.state-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.state-downloading { color: var(--color-state-downloading); }
.state-downloading .state-dot { background: var(--color-state-downloading); }
.state-completed { color: var(--color-state-completed); }
.state-completed .state-dot { background: var(--color-state-completed); }
.state-paused { color: var(--color-state-paused); }
.state-paused .state-dot { background: var(--color-state-paused); }
.state-failed { color: var(--color-state-failed); }
.state-failed .state-dot { background: var(--color-state-failed); }
.state-queued { color: var(--color-state-queued); }
.state-queued .state-dot { background: var(--color-state-queued); }

/* Status Bar */
.statusbar {
  height: 24px;
  background: #f6f7f9;
  border-top: 1px solid #dcdcdc;
  display: flex;
  align-items: center;
  font-size: 11.5px;
  color: #4b5563;
  padding: 0 4px;
  gap: 6px;
}

.status-pane {
  display: flex;
  align-items: center;
  padding: 0 8px;
  border-inline-end: 1px solid #dcdcdc;
  height: 18px;
  gap: 6px;
}

.status-pane:last-child {
  border-inline-end: none;
}

.status-pane-flex {
  flex: 1;
}

.speed-indicator {
  color: var(--color-turquoise-dark);
  font-weight: 700;
  font-family: var(--font-mono);
}

/* Context Menu Overlay */
.context-menu-popup {
  position: absolute;
  top: 245px;
  background: #ffffff;
  border: 1px solid #c9c9c9;
  border-radius: 6px;
  box-shadow: 0 8px 24px rgba(0,0,0,0.18), 0 2px 6px rgba(0,0,0,0.08);
  width: 250px;
  padding: 4px 0;
  font-size: 12.5px;
  z-index: 50;
}

.ctx-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 6px 14px;
  color: #222222;
  cursor: default;
}

.ctx-item:hover {
  background: #eef2f8;
  color: #0078d4;
}

.ctx-item-left {
  display: flex;
  align-items: center;
  gap: 10px;
}

.ctx-icon {
  width: 16px;
  height: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  color: #555555;
}

.ctx-shortcut {
  font-size: 11px;
  color: #888888;
}

.ctx-sep {
  height: 1px;
  background: #e5e5e5;
  margin: 4px 0;
}

/* Modal Dialog Overlay */
.dialog-backdrop {
  position: absolute;
  inset: 0;
  background: rgba(15, 23, 42, 0.45);
  backdrop-filter: blur(2px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.modal-dialog {
  width: 460px;
  background: #ffffff;
  border: 1px solid #b5b5b5;
  border-radius: 8px;
  box-shadow: 0 20px 40px rgba(0,0,0,0.28);
  overflow: hidden;
  display: flex;
  flex-direction: column;
}

.dialog-titlebar {
  height: 32px;
  background: #f8f9fa;
  border-bottom: 1px solid #e0e0e0;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
  font-weight: 600;
  font-size: 12.5px;
}

.dialog-body {
  padding: 16px 18px 12px 18px;
  display: flex;
  gap: 14px;
}

.dialog-icon-shield {
  width: 36px;
  height: 36px;
  border-radius: 50%;
  background: #fee2e2;
  color: #dc2626;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.dialog-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.dialog-prompt {
  font-size: 13px;
  color: #1f2937;
  font-weight: 500;
}

.dialog-filename-box {
  background: #f8fafc;
  border: 1px solid #e2e8f0;
  border-radius: 4px;
  padding: 6px 10px;
  font-family: var(--font-mono);
  font-size: 11.5px;
  color: #0b1a4e;
  word-break: break-all;
}

.checkbox-row {
  margin-top: 6px;
  display: flex;
  align-items: flex-start;
  gap: 8px;
  cursor: default;
}

.checkbox-row input {
  margin-top: 2px;
  accent-color: var(--color-turquoise);
  width: 14px;
  height: 14px;
}

.checkbox-label {
  font-size: 12.5px;
  font-weight: 600;
  color: #1f2937;
}

.checkbox-hint {
  font-size: 11px;
  color: #6b7280;
  margin-top: 2px;
}

.dialog-footer {
  background: #f8f9fa;
  border-top: 1px solid #e5e7eb;
  padding: 10px 16px;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
}

.btn {
  padding: 5px 16px;
  font-size: 12.5px;
  font-family: inherit;
  font-weight: 500;
  border-radius: 4px;
  cursor: default;
  border: 1px solid #c4c4c4;
  background: #fdfdfd;
}

.btn:hover {
  background: #f0f0f0;
  border-color: #a8a8a8;
}

.btn-primary {
  background: #0078d4;
  color: white;
  border-color: #006abc;
  font-weight: 600;
}

.btn-primary:hover {
  background: #006cc1;
}

.btn-danger {
  background: #dc2626;
  color: white;
  border-color: #b91c1c;
  font-weight: 600;
}

.btn-danger:hover {
  background: #b91c1c;
}
"""

SVG_ICONS = {
    "add": '<svg viewBox="0 0 24 24" fill="none" stroke="#00b6bd" stroke-width="2.5" width="26" height="26"><circle cx="12" cy="12" r="9" stroke="#00b6bd"/><path d="M12 8v8M8 12h8" stroke="#00b6bd"/></svg>',
    "resume": '<svg viewBox="0 0 24 24" fill="#16a34a" width="26" height="26"><polygon points="6 4 20 12 6 20 6 4"/></svg>',
    "pause": '<svg viewBox="0 0 24 24" fill="#d97706" width="26" height="26"><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></svg>',
    "pause_all": '<svg viewBox="0 0 24 24" fill="#6b7280" width="26" height="26"><rect x="4" y="4" width="3.5" height="16" rx="1"/><rect x="10" y="4" width="3.5" height="16" rx="1"/><rect x="16" y="4" width="3.5" height="16" rx="1"/></svg>',
    "delete": '<svg viewBox="0 0 24 24" fill="none" stroke="#dc2626" stroke-width="2" width="26" height="26"><path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2m3 0v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6h14zM10 11v6M14 11v6"/></svg>',
    "delete_completed": '<svg viewBox="0 0 24 24" fill="none" stroke="#4b5563" stroke-width="2" width="26" height="26"><path d="M3 6h18M8 6V4a2 2 0 012-2h4a2 2 0 012 2v2M5 6v14a2 2 0 002 2h10a2 2 0 002-2V6"/><path d="M9 13l2 2 4-4" stroke="#16a34a" stroke-width="2.5"/></svg>',
    "settings": '<svg viewBox="0 0 24 24" fill="none" stroke="#4b5563" stroke-width="2" width="26" height="26"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 00.33 1.82l.06.06a2 2 0 010 2.83 2 2 0 01-2.83 0l-.06-.06a1.65 1.65 0 00-1.82-.33 1.65 1.65 0 00-1 1.51V21a2 2 0 01-2 2 2 2 0 01-2-2v-.09A1.65 1.65 0 009 19.4a1.65 1.65 0 00-1.82.33l-.06.06a2 2 0 01-2.83 0 2 2 0 010-2.83l.06-.06a1.65 1.65 0 00.33-1.82 1.65 1.65 0 00-1.51-1H3a2 2 0 01-2-2 2 2 0 012-2h.09A1.65 1.65 0 004.6 9a1.65 1.65 0 00-.33-1.82l-.06-.06a2 2 0 010-2.83 2 2 0 012.83 0l.06.06a1.65 1.65 0 001.82.33H9a1.65 1.65 0 001-1.51V3a2 2 0 012-2 2 2 0 012 2v.09a1.65 1.65 0 001 1.51 1.65 1.65 0 001.82-.33l.06-.06a2 2 0 012.83 0 2 2 0 010 2.83l-.06.06a1.65 1.65 0 00-.33 1.82V9a1.65 1.65 0 001.51 1H21a2 2 0 012 2 2 2 0 01-2 2h-.09a1.65 1.65 0 00-1.51 1z"/></svg>',
    "schedule": '<svg viewBox="0 0 24 24" fill="none" stroke="#4b5563" stroke-width="2" width="26" height="26"><circle cx="12" cy="12" r="9"/><polyline points="12 6 12 12 16 14"/></svg>',
    "folder": '<svg viewBox="0 0 20 20" fill="#f59e0b" width="16" height="16"><path d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v6a2 2 0 01-2 2H4a2 2 0 01-2-2V6z"/></svg>',
    "folder_open": '<svg viewBox="0 0 20 20" fill="#f59e0b" width="16" height="16"><path fill-rule="evenodd" d="M2 6a2 2 0 012-2h4l2 2h6a2 2 0 012 2v1H4a2 2 0 00-1.994 1.851L2 18V6z" clip-rule="evenodd"/><path d="M4 9h14a1 1 0 01.99 1.141l-1.5 8A1 1 0 0116.5 19H3.5a1 1 0 01-.99-1.141l1.5-8A1 1 0 014 9z"/></svg>',
    "iso": '<svg viewBox="0 0 20 20" fill="#7c3aed" width="16" height="16"><circle cx="10" cy="10" r="8" fill="none" stroke="#7c3aed" stroke-width="1.8"/><circle cx="10" cy="10" r="3" fill="#7c3aed"/></svg>',
    "archive": '<svg viewBox="0 0 20 20" fill="#d97706" width="16" height="16"><path d="M4 4h12v4H4zM4 8v8h12V8"/><path d="M9 11h2"/></svg>',
    "program": '<svg viewBox="0 0 20 20" fill="#059669" width="16" height="16"><rect x="3" y="3" width="14" height="14" rx="2" fill="none" stroke="#059669" stroke-width="1.8"/><path d="M3 8h14M8 17V8"/></svg>',
    "document": '<svg viewBox="0 0 20 20" fill="#2563eb" width="16" height="16"><path d="M5 3h7l4 4v10H5z"/><path d="M12 3v4h4" fill="#93c5fd"/></svg>',
    "video": '<svg viewBox="0 0 20 20" fill="#dc2626" width="16" height="16"><rect x="2" y="4" width="16" height="12" rx="2" fill="none" stroke="#dc2626" stroke-width="1.8"/><polygon points="8 7 14 10 8 13" fill="#dc2626"/></svg>',
    "audio": '<svg viewBox="0 0 20 20" fill="#0284c7" width="16" height="16"><path d="M8 15V5l8-2v10"/><circle cx="6" cy="15" r="2.5"/><circle cx="14" cy="13" r="2.5"/></svg>',
}


def build_main_window_html(lang="ar", show_context_menu=False, show_delete_dialog=False):
    is_ar = (lang == "ar")
    dir_attr = "rtl" if is_ar else "ltr"
    
    # Text labels
    L = {
        "title": "سيل - SAYL Download Manager (إصدار المطورين - v0.1.0)" if is_ar else "SAYL Download Manager (Developer Build - v0.1.0)",
        "menu_file": "ملف" if is_ar else "File",
        "menu_tasks": "المهام" if is_ar else "Tasks",
        "menu_downloads": "التنزيلات" if is_ar else "Downloads",
        "menu_view": "عرض" if is_ar else "View",
        "menu_tools": "أدوات" if is_ar else "Tools",
        "menu_help": "مساعدة" if is_ar else "Help",
        "add": "إضافة رابط" if is_ar else "Add URL",
        "resume": "استئناف" if is_ar else "Resume",
        "stop": "إيقاف" if is_ar else "Stop",
        "stop_all": "إيقاف الكل" if is_ar else "Stop All",
        "delete": "حذف" if is_ar else "Delete",
        "delete_done": "حذف المكتمل" if is_ar else "Clean Done",
        "settings": "الإعدادات" if is_ar else "Settings",
        "schedule": "الجدولة" if is_ar else "Schedule",
        "search_ph": "بحث في التنزيلات..." if is_ar else "Search downloads...",
        "cat_header": "فئات التنزيل" if is_ar else "Categories",
        "cat_all": "الكل" if is_ar else "All Downloads",
        "cat_incomplete": "غير مكتمل" if is_ar else "Incomplete",
        "cat_complete": "مكتمل" if is_ar else "Completed",
        "cat_queue": "قائمة الانتظار" if is_ar else "Queue",
        "cat_cats": "الأنواع" if is_ar else "Types",
        "cat_compressed": "مضغوطة" if is_ar else "Compressed",
        "cat_docs": "مستندات" if is_ar else "Documents",
        "cat_programs": "برامج" if is_ar else "Programs",
        "cat_video": "فيديو" if is_ar else "Video",
        "cat_audio": "صوت" if is_ar else "Audio",
        "cat_other": "أخرى" if is_ar else "Other",
        "col_name": "الاسم" if is_ar else "Name",
        "col_size": "الحجم" if is_ar else "Size",
        "col_status": "الحالة / التقدم" if is_ar else "Status / Progress",
        "col_eta": "الوقت المتبقي" if is_ar else "Time Left",
        "col_speed": "سرعة النقل" if is_ar else "Transfer Rate",
        "col_date": "آخر محاولة" if is_ar else "Last Try",
        "col_desc": "الوصف / الرابط" if is_ar else "Description / URL",
        "status_active": "المهام النشطة: 2 تنزيل من 8" if is_ar else "Active Tasks: 2 of 8",
        "status_speed": "السرعة الإجمالية:" if is_ar else "Total Speed:",
        "status_disk": "المساحة الحرة على C: 142.8 GB" if is_ar else "Free Space on C: 142.8 GB",
        "status_ext": "متصل (إضافة المتصفح جاهزة)" if is_ar else "Connected (Browser Bridge Active)",
    }

    # Data Rows
    rows_data = [
        {
            "name": "ubuntu-24.04-desktop-amd64.iso",
            "icon": SVG_ICONS["iso"],
            "size": "5.80 GB",
            "type": "progress",
            "pct": 64,
            "text": "64% (3.71 GB)" if is_ar else "64% (3.71 GB)",
            "state_cls": "downloading",
            "eta": "04:18 د" if is_ar else "04:18 m",
            "speed": "14.2 MB/s",
            "date": "اليوم 10:14" if is_ar else "Today 10:14",
            "url": "https://releases.ubuntu.com/24.04/ubuntu-24.04-desktop-amd64.iso",
            "selected": True,
        },
        {
            "name": "Node-v20.18.0-x64.msi",
            "icon": SVG_ICONS["program"],
            "size": "31.4 MB",
            "type": "progress",
            "pct": 82,
            "text": "82% (25.7 MB)" if is_ar else "82% (25.7 MB)",
            "state_cls": "downloading",
            "eta": "00:08 ث" if is_ar else "00:08 s",
            "speed": "3.1 MB/s",
            "date": "اليوم 10:15" if is_ar else "Today 10:15",
            "url": "https://nodejs.org/dist/v20.18.0/node-v20.18.0-x64.msi",
            "selected": False,
        },
        {
            "name": "VSCodeUserSetup-x64-1.94.2.exe",
            "icon": SVG_ICONS["program"],
            "size": "98.5 MB",
            "type": "progress",
            "pct": 45,
            "text": "متوقف (45%)" if is_ar else "Paused (45%)",
            "state_cls": "paused",
            "eta": "—",
            "speed": "—",
            "date": "اليوم 09:30" if is_ar else "Today 09:30",
            "url": "https://update.code.visualstudio.com/1.94.2/win32-x64-user/stable",
            "selected": False,
        },
        {
            "name": "archive-backup-2026.zip",
            "icon": SVG_ICONS["archive"],
            "size": "245.0 MB",
            "type": "badge",
            "badge_text": "مكتمل" if is_ar else "Completed",
            "state_cls": "completed",
            "eta": "—",
            "speed": "—",
            "date": "أمس 18:22" if is_ar else "Yesterday 18:22",
            "url": "https://example.com/archive-backup-2026.zip",
            "selected": False,
        },
        {
            "name": "sayl-desktop-setup-v0.1.0.exe",
            "icon": SVG_ICONS["program"],
            "size": "6.22 MB",
            "type": "badge",
            "badge_text": "مكتمل" if is_ar else "Completed",
            "state_cls": "completed",
            "eta": "—",
            "speed": "—",
            "date": "24 أكتوبر" if is_ar else "Oct 24",
            "url": "https://sayl.site/releases/SAYL-Setup-x64-0.1.0.exe",
            "selected": False,
        },
        {
            "name": "specification-v3-final.pdf",
            "icon": SVG_ICONS["document"],
            "size": "14.2 MB",
            "type": "badge",
            "badge_text": "مكتمل" if is_ar else "Completed",
            "state_cls": "completed",
            "eta": "—",
            "speed": "—",
            "date": "23 أكتوبر" if is_ar else "Oct 23",
            "url": "https://sayl.site/docs/specification-v3.pdf",
            "selected": False,
        },
        {
            "name": "large-dataset-ai-models.tar.gz",
            "icon": SVG_ICONS["archive"],
            "size": "12.40 GB",
            "type": "badge",
            "badge_text": "في قائمة الانتظار" if is_ar else "Queued",
            "state_cls": "queued",
            "eta": "—",
            "speed": "—",
            "date": "اليوم 10:10" if is_ar else "Today 10:10",
            "url": "https://datasets.example.com/models.tar.gz",
            "selected": False,
        },
        {
            "name": "corrupted-stream-sample.bin",
            "icon": SVG_ICONS["other"] if "other" in SVG_ICONS else SVG_ICONS["iso"],
            "size": "50.0 MB",
            "type": "badge",
            "badge_text": "فشل (خطأ اتصال)" if is_ar else "Failed (Network Error)",
            "state_cls": "failed",
            "eta": "—",
            "speed": "—",
            "date": "اليوم 08:15" if is_ar else "Today 08:15",
            "url": "https://broken.example.com/sample.bin",
            "selected": False,
        },
    ]

    rows_html = ""
    for r in rows_data:
        sel_class = "selected" if r["selected"] else ""
        if r["type"] == "progress":
            status_cell = f"""
            <div class="cell-progress-bar">
              <div class="cell-progress-fill {r['state_cls']}" style="width: {r['pct']}%;"></div>
              <div class="cell-progress-text">{r['text']}</div>
            </div>
            """
        else:
            status_cell = f"""
            <span class="state-badge state-{r['state_cls']}">
              <span class="state-dot"></span>
              <span>{r['badge_text']}</span>
            </span>
            """
            
        rows_html += f"""
        <tr class="grid-row {sel_class}">
          <td>
            <div class="col-filename">
              <span class="file-type-icon">{r['icon']}</span>
              <span title="{r['name']}">{r['name']}</span>
            </div>
          </td>
          <td>{r['size']}</td>
          <td>{status_cell}</td>
          <td>{r['eta']}</td>
          <td>{r['speed']}</td>
          <td>{r['date']}</td>
          <td style="color:#6b7280; font-size:11.5px;" dir="ltr">{r['url']}</td>
        </tr>
        """

    context_menu_html = ""
    if show_context_menu:
        ctx_pos = "right: 280px;" if is_ar else "left: 280px;"
        context_menu_html = f"""
        <div class="context-menu-popup" style="{ctx_pos}">
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">▶</span>
              <strong>{"استئناف التنزيل" if is_ar else "Resume Download"}</strong>
            </div>
          </div>
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">⏸</span>
              <span>{"إيقاف مؤقت" if is_ar else "Pause Download"}</span>
            </div>
          </div>
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">🔄</span>
              <span>{"إعادة المحاولة" if is_ar else "Retry"}</span>
            </div>
          </div>
          <div class="ctx-sep"></div>
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">📂</span>
              <span>{"فتح الملف" if is_ar else "Open File"}</span>
            </div>
            <span class="ctx-shortcut">Enter</span>
          </div>
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">📁</span>
              <span>{"فتح المجلد المحتوي" if is_ar else "Open Folder"}</span>
            </div>
            <span class="ctx-shortcut">Ctrl+O</span>
          </div>
          <div class="ctx-sep"></div>
          <div class="ctx-item" style="color:#dc2626;">
            <div class="ctx-item-left">
              <span class="ctx-icon" style="color:#dc2626;">🗑</span>
              <span>{"حذف من القائمة..." if is_ar else "Delete..."}</span>
            </div>
            <span class="ctx-shortcut">Del</span>
          </div>
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">🗑</span>
              <span>{"حذف الملف من الجهاز..." if is_ar else "Delete File From Disk..."}</span>
            </div>
            <span class="ctx-shortcut">Shift+Del</span>
          </div>
          <div class="ctx-sep"></div>
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">🔗</span>
              <span>{"نسخ رابط التنزيل" if is_ar else "Copy URL"}</span>
            </div>
            <span class="ctx-shortcut">Ctrl+C</span>
          </div>
          <div class="ctx-item">
            <div class="ctx-item-left">
              <span class="ctx-icon">⚙</span>
              <span>{"خصائص التنزيل..." if is_ar else "Properties..."}</span>
            </div>
            <span class="ctx-shortcut">Alt+Enter</span>
          </div>
        </div>
        """

    delete_dialog_html = ""
    if show_delete_dialog:
        delete_dialog_html = f"""
        <div class="dialog-backdrop">
          <div class="modal-dialog">
            <div class="dialog-titlebar">
              <span>{"حذف التنزيل" if is_ar else "Delete Download"}</span>
              <button class="caption-btn close" style="width:26px; height:24px;">✕</button>
            </div>
            <div class="dialog-body">
              <div class="dialog-icon-shield">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" width="22" height="22">
                  <path d="M12 9v4m0 4h.01M20.61 5.99a2 2 0 00-1.61-1.61L12.7 3.12a2 2 0 00-1.4 0L4.99 4.38a2 2 0 00-1.6 1.61L2.12 12.3a2 2 0 00.58 1.69l6.3 6.3a2 2 0 002.83 0l6.3-6.3a2 2 0 00.58-1.69z"/>
                </svg>
              </div>
              <div class="dialog-content">
                <div class="dialog-prompt">
                  {"هل تريد بالتأكيد إزالة التنزيل التالي من القائمة؟" if is_ar else "Are you sure you want to remove the following download from the list?"}
                </div>
                <div class="dialog-filename-box">
                  ubuntu-24.04-desktop-amd64.iso (5.80 GB)
                </div>
                <label class="checkbox-row">
                  <input type="checkbox" id="del-file-check" checked>
                  <div>
                    <div class="checkbox-label">
                      {"إزالة وحذف الملف من الجهاز (إرسال إلى سلة المحذوفات)" if is_ar else "Also delete file from device (send to Recycle Bin)"}
                    </div>
                    <div class="checkbox-hint">
                      {"سيتم نقل الملف وأي ملفات أجزاء مؤقتة (.part) إلى سلة المحذوفات بأمان دون حذف نهائي." if is_ar else "Completed file and any temporary .part files will be moved to Recycle Bin safely."}
                    </div>
                  </div>
                </label>
              </div>
            </div>
            <div class="dialog-footer">
              <button class="btn btn-secondary">{"إلغاء" if is_ar else "Cancel"}</button>
              <button class="btn btn-danger">{"إزالة وحذف الملف" if is_ar else "Delete File and Remove"}</button>
            </div>
          </div>
        </div>
        """

    html = f"""<!DOCTYPE html>
<html lang="{lang}" dir="{dir_attr}">
<head>
  <meta charset="utf-8">
  <title>{L['title']}</title>
  <style>
    {COMMON_CSS}
  </style>
</head>
<body>
  <div class="window-frame">
    <!-- 1. Window Caption Titlebar -->
    <div class="titlebar">
      <div class="title-content">
        <svg viewBox="0 0 24 24" fill="#00b6bd" width="18" height="18">
          <circle cx="12" cy="12" r="10" fill="#0b1a4e"/>
          <path d="M12 7v10M8 12l4 4 4-4" stroke="#00b6bd" stroke-width="2" stroke-linecap="round"/>
        </svg>
        <span>{L['title']}</span>
      </div>
      <div class="window-caption-buttons">
        <button class="caption-btn">─</button>
        <button class="caption-btn">□</button>
        <button class="caption-btn close">✕</button>
      </div>
    </div>

    <!-- 2. Classic Menu Bar -->
    <div class="menubar">
      <div class="menu-item">{L['menu_file']}</div>
      <div class="menu-item">{L['menu_tasks']}</div>
      <div class="menu-item">{L['menu_downloads']}</div>
      <div class="menu-item">{L['menu_view']}</div>
      <div class="menu-item">{L['menu_tools']}</div>
      <div class="menu-item">{L['menu_help']}</div>
    </div>

    <!-- 3. Classic Large Icon Toolbar -->
    <div class="toolbar">
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['add']}</div>
        <span class="toolbar-label">{L['add']}</span>
      </div>
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['resume']}</div>
        <span class="toolbar-label">{L['resume']}</span>
      </div>
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['pause']}</div>
        <span class="toolbar-label">{L['stop']}</span>
      </div>
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['pause_all']}</div>
        <span class="toolbar-label">{L['stop_all']}</span>
      </div>
      <div class="toolbar-sep"></div>
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['delete']}</div>
        <span class="toolbar-label">{L['delete']}</span>
      </div>
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['delete_completed']}</div>
        <span class="toolbar-label">{L['delete_done']}</span>
      </div>
      <div class="toolbar-sep"></div>
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['settings']}</div>
        <span class="toolbar-label">{L['settings']}</span>
      </div>
      <div class="toolbar-btn">
        <div class="toolbar-icon">{SVG_ICONS['schedule']}</div>
        <span class="toolbar-label">{L['schedule']}</span>
      </div>

      <div class="toolbar-search">
        <input type="text" class="search-input" placeholder="{L['search_ph']}">
      </div>
    </div>

    <!-- 4. Main Body: Splitter + Category Tree + Dense Data Grid -->
    <div class="main-content">
      <!-- Left Tree View (IDM style) -->
      <div class="sidebar-tree">
        <div class="tree-section-header">{L['cat_header']}</div>
        <div class="tree-node active">
          <span class="node-icon">{SVG_ICONS['folder_open']}</span>
          <span class="node-title">{L['cat_all']}</span>
          <span class="node-badge">14</span>
        </div>
        <div class="tree-node">
          <span class="node-icon">{SVG_ICONS['folder']}</span>
          <span class="node-title">{L['cat_incomplete']}</span>
          <span class="node-badge">4</span>
        </div>
        <div class="tree-node">
          <span class="node-icon">{SVG_ICONS['folder']}</span>
          <span class="node-title">{L['cat_complete']}</span>
          <span class="node-badge">8</span>
        </div>
        <div class="tree-node">
          <span class="node-icon">{SVG_ICONS['folder']}</span>
          <span class="node-title">{L['cat_queue']}</span>
          <span class="node-badge">2</span>
        </div>

        <div class="tree-section-header" style="margin-top: 10px;">{L['cat_cats']}</div>
        <div class="tree-node tree-subnode">
          <span class="node-icon">{SVG_ICONS['archive']}</span>
          <span class="node-title">{L['cat_compressed']}</span>
          <span class="node-badge">3</span>
        </div>
        <div class="tree-node tree-subnode">
          <span class="node-icon">{SVG_ICONS['document']}</span>
          <span class="node-title">{L['cat_docs']}</span>
          <span class="node-badge">2</span>
        </div>
        <div class="tree-node tree-subnode">
          <span class="node-icon">{SVG_ICONS['program']}</span>
          <span class="node-title">{L['cat_programs']}</span>
          <span class="node-badge">4</span>
        </div>
        <div class="tree-node tree-subnode">
          <span class="node-icon">{SVG_ICONS['video']}</span>
          <span class="node-title">{L['cat_video']}</span>
          <span class="node-badge">3</span>
        </div>
        <div class="tree-node tree-subnode">
          <span class="node-icon">{SVG_ICONS['audio']}</span>
          <span class="node-title">{L['cat_audio']}</span>
          <span class="node-badge">1</span>
        </div>
        <div class="tree-node tree-subnode">
          <span class="node-icon">{SVG_ICONS['folder']}</span>
          <span class="node-title">{L['cat_other']}</span>
          <span class="node-badge">1</span>
        </div>
      </div>

      <!-- Right Data Grid -->
      <div class="grid-container">
        <table class="data-grid">
          <thead>
            <tr class="grid-header">
              <th style="width: 280px;">{L['col_name']}</th>
              <th style="width: 85px;">{L['col_size']}</th>
              <th style="width: 175px;">{L['col_status']}</th>
              <th style="width: 95px;">{L['col_eta']}</th>
              <th style="width: 95px;">{L['col_speed']} <span class="sort-arrow">▼</span></th>
              <th style="width: 110px;">{L['col_date']}</th>
              <th>{L['col_desc']}</th>
            </tr>
          </thead>
          <tbody>
            {rows_html}
          </tbody>
        </table>
      </div>
    </div>

    <!-- 5. Bottom Status Bar -->
    <div class="statusbar">
      <div class="status-pane">
        <span>{L['status_active']}</span>
      </div>
      <div class="status-pane">
        <span>{L['status_speed']}</span>
        <span class="speed-indicator">17.3 MB/s</span>
      </div>
      <div class="status-pane">
        <span>{L['status_disk']}</span>
      </div>
      <div class="status-pane status-pane-flex">
        <span>{L['status_ext']}</span>
      </div>
    </div>

    <!-- Optional Overlays -->
    {context_menu_html}
    {delete_dialog_html}
  </div>
</body>
</html>"""
    return html


def capture_screenshot(html_content, output_png_path):
    temp_html = REPO_ROOT / "docs" / "ui-mockups" / "native" / f"_temp_{output_png_path.stem}.html"
    temp_html.write_text(html_content, encoding="utf-8")
    
    url = f"file:///{temp_html.as_posix()}"
    args = [
        CHROME_PATH,
        "--headless",
        "--disable-gpu",
        "--window-size=1280,820",
        "--device-scale-factor=1.5",
        f"--screenshot={str(output_png_path)}",
        url,
    ]
    
    try:
        res = subprocess.run(args, capture_output=True, timeout=30)
        if res.returncode != 0:
            print(f"Warning: chrome returned {res.returncode}: {res.stderr.decode('utf-8', errors='ignore')}")
    finally:
        if temp_html.exists():
            temp_html.unlink()
            
    if output_png_path.exists():
        print(f"Generated: {output_png_path.name} ({output_png_path.stat().st_size} bytes)")
    else:
        print(f"Failed to generate: {output_png_path.name}")


def main():
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    
    # 1. Main Window RTL (Arabic)
    html_main = build_main_window_html(lang="ar", show_context_menu=False, show_delete_dialog=False)
    capture_screenshot(html_main, OUTPUT_DIR / "main-window.png")
    
    # 2. Main Window LTR (English)
    html_main_en = build_main_window_html(lang="en", show_context_menu=False, show_delete_dialog=False)
    capture_screenshot(html_main_en, OUTPUT_DIR / "main-window-en.png")
    
    # 3. Context Menu
    html_ctx = build_main_window_html(lang="ar", show_context_menu=True, show_delete_dialog=False)
    capture_screenshot(html_ctx, OUTPUT_DIR / "context-menu.png")
    
    # 4. Delete Dialog
    html_del = build_main_window_html(lang="ar", show_context_menu=False, show_delete_dialog=True)
    capture_screenshot(html_del, OUTPUT_DIR / "delete-dialog.png")
    
    print("All mockups generated successfully in:", OUTPUT_DIR)


if __name__ == "__main__":
    main()
