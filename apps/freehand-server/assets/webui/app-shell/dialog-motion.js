const DIALOG_CLOSE_MS = 150;
const pendingDialogCloses = new WeakMap();

function cancelPendingDialogClose(dialog, windowRef = window) {
  const pendingClose = pendingDialogCloses.get(dialog);
  if (!pendingClose) return;
  windowRef.clearTimeout(pendingClose.timeoutId);
  pendingDialogCloses.delete(dialog);
}

export function openAnimatedDialog(dialog, windowRef = window) {
  if (!dialog) return false;
  cancelPendingDialogClose(dialog, windowRef);
  dialog.classList.remove("is-closing", "is-open");
  if (typeof dialog.showModal === "function" && !dialog.open) {
    dialog.showModal();
  }
  void dialog.offsetWidth;
  windowRef.requestAnimationFrame(() => dialog.classList.add("is-open"));
  return true;
}

export function closeAnimatedDialog(dialog, onClose, windowRef = window) {
  if (!dialog) return;
  if (pendingDialogCloses.has(dialog)) return;
  if (!dialog.open) {
    if (onClose) onClose();
    return;
  }
  dialog.classList.remove("is-open");
  dialog.classList.add("is-closing");
  const timeoutId = windowRef.setTimeout(() => {
    dialog.classList.remove("is-closing");
    dialog.close();
    pendingDialogCloses.delete(dialog);
    if (onClose) onClose();
  }, DIALOG_CLOSE_MS);
  pendingDialogCloses.set(dialog, { timeoutId });
}

export function bindAnimatedDialogCancel(dialog, onCancel, windowRef = window) {
  if (!dialog) return;
  dialog.addEventListener("cancel", (event) => {
    event.preventDefault();
    closeAnimatedDialog(dialog, onCancel, windowRef);
  });
}
