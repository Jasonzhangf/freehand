export function selectedNewSessionKind(context) {
  const checked = context.dom.form
    ? context.dom.form.querySelector('input[name="new-session-kind"]:checked')
    : null;
  return (checked && checked.value) || context.state.newSessionKind || 'conversation';
}

export function syncNewSessionDialogMode(context) {
  const kind = selectedNewSessionKind(context);
  context.state.newSessionKind = kind;
  if (context.dom.dialog) {
    context.dom.dialog.dataset.kind = kind;
  }
  if (context.dom.confirmButton) {
    context.dom.confirmButton.textContent = kind === 'task' ? '创建任务会话' : '创建会话';
  }
}

export function openNewSessionSurface(kind = 'conversation', context) {
  context.dispatchEdge('home.open_new');
  context.state.newSessionKind = kind === 'task' ? 'task' : 'conversation';
  if (!context.dom.dialog || !context.dom.form) {
    if (context.state.newSessionKind === 'task') {
      context.startNewTask().catch((error) => {
        context.setCommandStatus(`新建任务失败：${error.message}`, { stickyMs: 8000 });
      });
    } else {
      context.startNewConversation().catch((error) => {
        context.setCommandStatus(`新建会话失败：${error.message}`, { stickyMs: 8000 });
      });
    }
    return;
  }
  const radio = context.dom.form.querySelector(`input[name="new-session-kind"][value="${context.state.newSessionKind}"]`);
  if (radio) radio.checked = true;
  if (context.dom.cwdInput) {
    context.dom.cwdInput.value = context.selectedWorkspaceCwd();
  }
  syncNewSessionDialogMode(context);
  context.dom.dialog.showModal();
  window.setTimeout(() => {
    if (context.state.newSessionKind === 'task') {
      (context.dom.browseButton || context.dom.cwdInput || context.dom.confirmButton)?.focus();
    } else {
      context.dom.confirmButton?.focus();
    }
  }, 0);
}

export function closeNewSessionSurface(context) {
  if (context.dom.dialog && context.dom.dialog.open) {
    context.dom.dialog.close();
  }
  if (context.state.route === 'new_session') {
    context.dispatchEdge('root.open_home');
    context.renderAll();
  }
}

export async function chooseNewTaskDirectory(context) {
  const firstPreset = context.dom.pathPresets?.querySelector('.path-preset-button');
  if (firstPreset) {
    firstPreset.focus();
    context.setCommandStatus('选择一个目录预设，或手动输入路径', { stickyMs: 5000 });
    return;
  }
  context.dom.cwdInput?.focus();
  context.setCommandStatus('请输入任务目标目录', { stickyMs: 5000 });
}

export async function submitNewSessionSurface(context) {
  const kind = selectedNewSessionKind(context);
  if (kind === 'task') {
    const cwd = context.normalizeCwd(context.dom.cwdInput && context.dom.cwdInput.value);
    if (!cwd) {
      context.setCommandStatus('新建任务需要目标目录', { stickyMs: 6000 });
      context.dom.cwdInput?.focus();
      return;
    }
    context.setSelectedCwd(cwd);
    closeNewSessionSurface(context);
    await context.startNewTask({ cwd });
    return;
  }
  closeNewSessionSurface(context);
  await context.startNewConversation();
}
