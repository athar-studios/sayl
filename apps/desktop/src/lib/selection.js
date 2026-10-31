/**
 * Multi-selection utilities for task lists and tables.
 * Supports single click, Ctrl/Cmd click (toggle), Shift click (range),
 * and keyboard navigation / Select All.
 */

/**
 * Handles an item click with optional modifier keys (Ctrl/Cmd, Shift).
 *
 * @param {Object} params
 * @param {Array<{id: string}>} params.items - Ordered list of visible tasks
 * @param {Array<string>} [params.selectedIds=[]] - Current list of selected task IDs
 * @param {string} params.clickedId - ID of clicked task
 * @param {Object} [params.event={}] - Click/pointer event with modifier keys
 * @param {string|null} [params.lastAnchorId=null] - Last clicked anchor ID for Shift ranges
 * @returns {{ selectedIds: string[], lastAnchorId: string|null }}
 */
export function handleItemClick({
  items = [],
  selectedIds = [],
  clickedId,
  event = {},
  lastAnchorId = null,
}) {
  if (!clickedId) {
    return { selectedIds, lastAnchorId };
  }

  const clickedIndex = items.findIndex((item) => item.id === clickedId);
  if (clickedIndex === -1) {
    return { selectedIds, lastAnchorId };
  }

  const isCtrl = Boolean(event.ctrlKey || event.metaKey);
  const isShift = Boolean(event.shiftKey);

  // 1. Shift + Click (Range selection)
  if (isShift) {
    const anchorIndex = lastAnchorId
      ? items.findIndex((item) => item.id === lastAnchorId)
      : -1;

    if (anchorIndex !== -1) {
      const minIdx = Math.min(anchorIndex, clickedIndex);
      const maxIdx = Math.max(anchorIndex, clickedIndex);
      const rangeIds = items.slice(minIdx, maxIdx + 1).map((item) => item.id);

      if (isCtrl) {
        // Union range with existing selection
        const combined = new Set([...selectedIds, ...rangeIds]);
        return {
          selectedIds: Array.from(combined),
          lastAnchorId,
        };
      }

      return {
        selectedIds: rangeIds,
        lastAnchorId,
      };
    }

    // No valid anchor, fall through to single selection
    return {
      selectedIds: [clickedId],
      lastAnchorId: clickedId,
    };
  }

  // 2. Ctrl / Cmd + Click (Toggle selection)
  if (isCtrl) {
    const exists = selectedIds.includes(clickedId);
    const nextSelected = exists
      ? selectedIds.filter((id) => id !== clickedId)
      : [...selectedIds, clickedId];

    return {
      selectedIds: nextSelected,
      lastAnchorId: clickedId,
    };
  }

  // 3. Regular Click (Select single item)
  return {
    selectedIds: [clickedId],
    lastAnchorId: clickedId,
  };
}

/**
 * Returns IDs of all given items.
 *
 * @param {Array<{id: string}>} items
 * @returns {string[]}
 */
export function selectAll(items = []) {
  return items.map((item) => item.id);
}

/**
 * Returns an empty selection.
 *
 * @returns {string[]}
 */
export function clearSelection() {
  return [];
}

/**
 * Toggles single ID presence in a selection array.
 *
 * @param {string[]} selectedIds
 * @param {string} id
 * @returns {string[]}
 */
export function toggleSelect(selectedIds = [], id) {
  if (!id) return selectedIds;
  return selectedIds.includes(id)
    ? selectedIds.filter((item) => item !== id)
    : [...selectedIds, id];
}

/**
 * Moves selection up or down by 1 item via keyboard arrow keys.
 *
 * @param {Object} params
 * @param {Array<{id: string}>} params.items - Ordered list of visible tasks
 * @param {Array<string>} [params.selectedIds=[]] - Current selected IDs
 * @param {"up"|"down"} params.direction - Arrow navigation direction
 * @param {boolean} [params.shiftKey=false] - Whether Shift is held to extend selection
 * @param {string|null} [params.lastAnchorId=null] - Anchor ID for range extension
 * @returns {{ selectedIds: string[], lastAnchorId: string|null, activeId: string|null }}
 */
export function moveSelection({
  items = [],
  selectedIds = [],
  direction = "down",
  shiftKey = false,
  lastAnchorId = null,
}) {
  if (!items.length) {
    return { selectedIds: [], lastAnchorId: null, activeId: null };
  }

  const delta = direction === "up" ? -1 : 1;
  const lastActiveId = selectedIds[selectedIds.length - 1] || lastAnchorId;
  const currentIndex = lastActiveId
    ? items.findIndex((item) => item.id === lastActiveId)
    : -1;

  let targetIndex;
  if (currentIndex === -1) {
    targetIndex = direction === "up" ? items.length - 1 : 0;
  } else {
    targetIndex = Math.max(0, Math.min(items.length - 1, currentIndex + delta));
  }

  const targetId = items[targetIndex].id;

  if (shiftKey) {
    const anchorId = lastAnchorId || items[0].id;
    const anchorIndex = items.findIndex((item) => item.id === anchorId);
    const minIdx = Math.min(anchorIndex, targetIndex);
    const maxIdx = Math.max(anchorIndex, targetIndex);
    const rangeIds = items.slice(minIdx, maxIdx + 1).map((item) => item.id);

    return {
      selectedIds: rangeIds,
      lastAnchorId: anchorId,
      activeId: targetId,
    };
  }

  return {
    selectedIds: [targetId],
    lastAnchorId: targetId,
    activeId: targetId,
  };
}
