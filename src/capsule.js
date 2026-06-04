const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// State
let sessions = [];
let settings = {};
let providers = [];
let showSettings = false;
let timerInterval = null;
const ROW_HEIGHT = 48;
const TITLE_HEIGHT = 36;
const EMPTY_HEIGHT = 52;
const SETTINGS_HEIGHT = 400;
const EXPANDED_WIDTH = 360;
const COLLAPSED_HEIGHT = 34;
let lastHeight = null;
let lastWidth = null;

// Compact-mode display state
let displayMode = 'expanded'; // 'collapsed' | 'expanded'
let expandedByUser = false; // user clicked the pill to expand
let manualEngage = false; // user opened it from the tray; keep interactive until blur
let lastIdleKey = null;

// Window controls
document.getElementById('closeBtn').addEventListener('click', () => {
  invoke('minimize_to_tray');
});

// Collapse back to the compact pill
document.getElementById('collapseBtn').addEventListener('click', (e) => {
  e.stopPropagation();
  showSettings = false;
  settingsBtn.classList.remove('open');
  settingsPanel.style.display = 'none';
  expandedByUser = false;
  manualEngage = false;
  setupBannerNames = null;
  updateDisplay();
});

// DOM refs
const capsule = document.getElementById('capsule');
const sessionList = document.getElementById('sessionList');
const settingsPanel = document.getElementById('settingsPanel');
const settingsBtn = document.getElementById('settingsBtn');
const soundToggle = document.getElementById('soundToggle');
const providerList = document.getElementById('providerList');


const textScales = {
  small: 0.85,
  medium: 1.0,
  large: 1.15,
};

// Drag and drop state (mouse-event based)
let dragState = null; // { sessionId, startY, rowEl, placeholder }
let dragOverSessionId = null;

const stateLabels = {
  idle: 'Idle',
  working: 'Working',
  waitingForUser: 'Waiting',
};

// --- Initialization ---

async function init() {
  [settings, providers, sessions] = await Promise.all([
    invoke('get_settings'),
    invoke('get_providers'),
    invoke('get_sessions'),
  ]);

  setupDragListeners();
  setupRowClickDelegation();
  setupExpandListeners();
  applySettings();
  render();
  startTimer();

  await listen('sessions-changed', (event) => {
    sessions = event.payload;
    if (!dragState) {
      render();
    }
  });

  await listen('settings-changed', (event) => {
    settings = event.payload;
    applySettings();
    if (showSettings) renderSettings();
    render();
  });

  await listen('providers-changed', (event) => {
    providers = event.payload;
    if (showSettings) renderProviders();
  });

  await listen('show-settings', () => {
    showSettings = true;
    settingsBtn.classList.add('open');
    renderSettings();
    updateDisplay();
  });

  await listen('play-sound', () => {
    playCompleteSound();
  });

  await listen('play-waiting-sound', () => {
    playWaitingSound();
  });

  // Shown via the tray while idle/click-through: keep it interactive until the
  // pointer leaves again.
  await listen('force-interactive', () => {
    manualEngage = true;
    updateDisplay();
  });

  await listen('unconfigured-providers', (event) => {
    const names = event.payload;
    if (names && names.length > 0) {
      showSetupBanner(names);
    }
  });

  settingsBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    showSettings = !showSettings;
    settingsBtn.classList.toggle('open', showSettings);
    if (showSettings) {
      renderSettings();
    } else {
      settingsPanel.style.display = 'none';
    }
    updateDisplay();
  });
}

// --- Compact mode: collapse / expand ---

function setupExpandListeners() {
  // The ▼ button is the only click target while collapsed; the rest of the
  // pill is a drag region for moving the window.
  document.getElementById('pillExpandBtn').addEventListener('click', (e) => {
    e.stopPropagation();
    expandedByUser = true;
    updateDisplay();
  });

  // Collapse again when the window loses focus (user clicked elsewhere).
  window.addEventListener('blur', () => {
    expandedByUser = false;
    manualEngage = false;
    updateDisplay();
  });
}

function aggregateState() {
  const counts = { working: 0, waitingForUser: 0, idle: 0 };
  sessions.forEach(s => { if (counts[s.state] !== undefined) counts[s.state]++; });
  return counts;
}

function isIdleAggregate() {
  const c = aggregateState();
  return c.working === 0 && c.waitingForUser === 0;
}

function needsSetup() {
  return setupBannerNames && setupBannerNames.length > 0 && !providers.some(p => p.installed);
}

function shouldExpand() {
  if (!settings.compactMode) return true;
  if (showSettings) return true;
  if (manualEngage) return true;
  if (expandedByUser) return true;
  if (needsSetup()) return true;
  return aggregateState().waitingForUser > 0; // waiting needs the user's attention
}

// Groups shown in the collapsed pill: always all three states (when there are
// sessions), in a fixed order so positions don't jump around.
function pillGroupsData() {
  if (sessions.length === 0) return [];
  const c = aggregateState();
  return [
    { state: 'working', cls: 'is-working', count: c.working },
    { state: 'waitingForUser', cls: 'is-waiting', count: c.waitingForUser },
    { state: 'idle', cls: 'is-idle', count: c.idle },
  ];
}

function collapsedWidth() {
  const n = pillGroupsData().length;
  if (n === 0) return 104; // icon + "Idle" + expand button
  return 60 + n * 34; // icon + groups + expand button
}

function renderCollapsedPill() {
  const dotColors = getDotColors();
  const groups = pillGroupsData();
  const el = document.getElementById('pillGroups');
  if (groups.length === 0) {
    el.innerHTML = '<span class="pill-empty">Idle</span>';
    return;
  }
  el.innerHTML = groups.map(g => {
    const color = dotColors[g.state] || dotColors.idle;
    const active = g.count > 0 && (g.state === 'working' || g.state === 'waitingForUser');
    const cls = ['pill-group', active ? g.cls : '', g.count === 0 ? 'is-empty' : ''].filter(Boolean).join(' ');
    return `<div class="${cls}" data-tauri-drag-region>
      <div class="pill-dot-container">
        <div class="pill-dot" style="background:${color}"></div>
        <div class="pill-dot-pulse" style="background:${color}"></div>
      </div>
      <span class="pill-count">${g.count}</span>
    </div>`;
  }).join('');
}

function updateDisplay() {
  const expand = shouldExpand();
  displayMode = expand ? 'expanded' : 'collapsed';
  capsule.classList.toggle('collapsed', !expand);

  // The collapse button only makes sense when compact mode is enabled.
  const collapseBtn = document.getElementById('collapseBtn');
  if (collapseBtn) collapseBtn.style.display = settings.compactMode ? '' : 'none';

  if (expand) {
    renderSessionList();
  } else {
    renderCollapsedPill();
  }
  applyGeometry();
  applyIdleAppearance();
}

function applyIdleAppearance() {
  const engaged = showSettings || manualEngage || expandedByUser;
  const idle = isIdleAggregate();
  const dimmed = settings.dimWhenIdle && idle && !engaged && displayMode === 'collapsed';
  const clickThrough = settings.clickThroughWhenIdle && idle && !engaged && displayMode === 'collapsed';
  const key = `${dimmed}-${clickThrough}`;
  if (key === lastIdleKey) return;
  lastIdleKey = key;
  invoke('set_idle_mode', { dimmed, clickThrough }).catch(e => console.error('set_idle_mode failed:', e));
}

// --- Rendering ---

function render() {
  updateDisplay();
}

let setupBannerNames = null;

function showSetupBanner(names) {
  // Only show if no providers are configured yet
  const anyInstalled = providers.some(p => p.installed);
  if (anyInstalled) return;
  setupBannerNames = names;
  render();
}

function renderSetupBanner() {
  if (!setupBannerNames || setupBannerNames.length === 0) return '';
  // Hide banner once any provider is configured
  if (providers.some(p => p.installed)) {
    setupBannerNames = null;
    return '';
  }
  return `<div class="setup-banner">
    <div class="setup-banner-text">Configure integrations to start monitoring</div>
    <button class="setup-banner-btn" id="openSetupBtn">Setup</button>
  </div>`;
}

function renderSessionList() {
  if (sessions.length === 0) {
    const banner = renderSetupBanner();
    sessionList.innerHTML = '<div class="no-sessions">No active sessions</div>' + banner;
    const setupBtn = document.getElementById('openSetupBtn');
    if (setupBtn) {
      setupBtn.onclick = () => {
        showSettings = true;
        settingsBtn.classList.add('open');
        renderSettings();
        updateDisplay();
      };
    }
    return;
  }

  // Build badge lookup from providers
  const badgeMap = {};
  providers.forEach(p => { badgeMap[p.id] = { label: p.badgeLabel, color: p.badgeColor }; });

  let lastPinnedIndex = -1;
  for (let i = sessions.length - 1; i >= 0; i--) {
    if (sessions[i].pinned) { lastPinnedIndex = i; break; }
  }
  const hasPinnedSessions = lastPinnedIndex >= 0;

  const dotColors = getDotColors();

  sessionList.innerHTML = sessions.map((s, index) => {
    const dotColor = dotColors[s.state] || dotColors.idle;
    const stateClass = 'row-state-' + s.state;
    const promptText = s.lastPrompt ? truncate(s.lastPrompt, 40) : (s.lastToolName ? `Tool: ${s.lastToolName}` : '');
    const badge = badgeMap[s.source] || { label: s.source?.slice(0, 2).toUpperCase() || '??', color: '#71717a' };
    const isPinned = s.pinned;
    const pinIcon = `<svg class="pin-icon ${isPinned ? 'pinned' : 'unpinned'}" viewBox="0 0 24 24" width="14" height="14" fill="currentColor">
        <path d="M16 12V4H17V2H7V4H8V12L6 14V16H11.2V22H12.8V16H18V14L16 12Z"/>
      </svg>`;
    
    // Add separator after last pinned session
    const separator = hasPinnedSessions && index === lastPinnedIndex ? 
      '<div class="pinned-separator"></div>' : '';
    
    const pinnedClass = isPinned ? 'pinned-row' : '';
    
    return `
      <div class="session-row ${stateClass} ${pinnedClass}" data-id="${s.id}">
        <div class="row-pin ${hasPinnedSessions ? '' : 'pin-hidden'}">
          ${pinIcon}
        </div>
        <div class="row-dot-container">
          <div class="row-dot" style="background: ${dotColor}"></div>
          <div class="row-dot-pulse" style="background: ${dotColor}"></div>
        </div>
        <div class="row-info">
          <div class="row-project"><span class="source-badge" style="background: ${badge.color}20; color: ${badge.color}">${badge.label}</span>${escapeHtml(s.projectName)}<span class="debug-pid"> [${s.pid || '?'}]</span></div>
          ${promptText ? `<div class="row-prompt">${escapeHtml(promptText)}</div>` : ''}
        </div>
        <span class="row-state">${stateLabels[s.state] || s.state}</span>
        <span class="row-timer">${formatElapsed(s.startTimeMs)}</span>
        <div class="row-close">
          <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
          </svg>
        </div>
      </div>
      ${separator}
    `;
  }).join('');

}

function togglePin(sessionId) {
  const session = sessions.find(s => s.id === sessionId);
  if (!session) return;
  
  if (session.pinned) {
    unpinSession(sessionId);
  } else {
    pinSession(sessionId);
  }
}

function pinSession(sessionId) {
  invoke('pin_session', { sessionId }).catch(e => console.error('pin_session failed:', e));
}

function unpinSession(sessionId) {
  invoke('unpin_session', { sessionId }).catch(e => console.error('unpin_session failed:', e));
}

function removeSession(sessionId) {
  invoke('remove_session', { sessionId }).catch(e => console.error('remove_session failed:', e));
}

function reorderPinnedSessions(orderedIds) {
  invoke('reorder_pinned_sessions', { orderedIds }).catch(e => console.error('reorder failed:', e));
}

// --- Mouse-based drag reorder for pinned sessions ---

function setupDragListeners() {
  let startY = 0;
  let dragging = false;
  let dropTargets = []; // cached { id, top, bottom, el }
  const DRAG_THRESHOLD = 4;

  sessionList.addEventListener('mousedown', (e) => {
    if (e.target.closest('.row-pin')) return;
    const row = e.target.closest('.session-row.pinned-row');
    if (!row || e.button !== 0) return;

    startY = e.clientY;
    dragging = false;

    const sessionId = row.dataset.id;

    const cleanup = () => {
      document.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('mouseup', onMouseUp);
      window.removeEventListener('blur', onBlur);
      if (!dragging) return;
      row.classList.remove('dragging');
      sessionList.querySelectorAll('.drag-over').forEach(el => el.classList.remove('drag-over'));
      document.body.style.cursor = '';
      dragState = null;
      dragOverSessionId = null;
      dropTargets = [];
    };

    const onMouseMove = (e) => {
      if (!dragging && Math.abs(e.clientY - startY) >= DRAG_THRESHOLD) {
        dragging = true;
        dragState = { sessionId };
        row.classList.add('dragging');
        // Cache rects once at drag start
        dropTargets = [];
        sessionList.querySelectorAll('.session-row.pinned-row').forEach(r => {
          if (r.dataset.id === sessionId) return;
          const rect = r.getBoundingClientRect();
          dropTargets.push({ id: r.dataset.id, top: rect.top, bottom: rect.bottom, el: r });
        });
      }

      if (!dragging) return;

      const target = dropTargets.find(t => e.clientY >= t.top && e.clientY <= t.bottom);
      sessionList.querySelectorAll('.drag-over').forEach(el => el.classList.remove('drag-over'));
      dragOverSessionId = null;

      if (target) {
        target.el.classList.add('drag-over');
        dragOverSessionId = target.id;
        document.body.style.cursor = 'grabbing';
      } else {
        document.body.style.cursor = 'not-allowed';
      }
    };

    const onMouseUp = () => {
      const targetId = dragOverSessionId;
      cleanup();
      if (targetId && targetId !== sessionId) {
        applyReorder(sessionId, targetId);
      }
    };

    const onBlur = () => cleanup();

    document.addEventListener('mousemove', onMouseMove);
    document.addEventListener('mouseup', onMouseUp);
    window.addEventListener('blur', onBlur);
  });
}

function applyReorder(draggedId, targetId) {
  const pinnedIds = sessions.filter(s => s.pinned).map(s => s.id);
  const draggedIndex = pinnedIds.indexOf(draggedId);
  const targetIndex = pinnedIds.indexOf(targetId);
  if (draggedIndex === -1 || targetIndex === -1) return;

  pinnedIds.splice(draggedIndex, 1);
  pinnedIds.splice(targetIndex, 0, draggedId);

  // Optimistic local update
  const newSessions = [...sessions];
  pinnedIds.forEach((id, index) => {
    const session = newSessions.find(s => s.id === id);
    if (session) session.pinOrder = index;
  });

  const reordered = pinnedIds.map(id => newSessions.find(s => s.id === id));
  const unpinned = newSessions.filter(s => !s.pinned);
  sessions = [...reordered, ...unpinned];

  renderSessionList();
  resizeWindow();
  reorderPinnedSessions(pinnedIds);
}

function setupRowClickDelegation() {
  sessionList.addEventListener('click', (e) => {
    const row = e.target.closest('.session-row');
    if (!row) return;
    const id = row.dataset.id;

    if (e.target.closest('.row-close')) {
      e.stopPropagation();
      removeSession(id);
      return;
    }
    if (e.target.closest('.row-pin')) {
      e.stopPropagation();
      togglePin(id);
      return;
    }
  });
}

function renderSettings() {
  settingsPanel.style.display = '';

  document.querySelectorAll('.theme-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.theme === settings.theme);
    btn.onclick = () => setSetting('theme', btn.dataset.theme);
  });

  bindColorPicker('colorWorking');
  bindColorPicker('colorWaiting');
  bindColorPicker('colorIdle');

  document.querySelectorAll('.size-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.size === settings.textSize);
    btn.onclick = () => setSetting('textSize', btn.dataset.size);
  });

  soundToggle.textContent = settings.soundOnComplete ? 'On' : 'Off';
  soundToggle.classList.toggle('on', settings.soundOnComplete);
  soundToggle.onclick = () => setSetting('soundOnComplete', (!settings.soundOnComplete).toString());

  bindToggle('compactToggle', 'compactMode', settings.compactMode);
  bindToggle('dimToggle', 'dimWhenIdle', settings.dimWhenIdle);
  bindToggle('clickThroughToggle', 'clickThroughWhenIdle', settings.clickThroughWhenIdle);

  renderProviders();

  document.getElementById('resetBtn').onclick = async () => {
    await invoke('reset_settings');
  };
}

function renderProviders() {
  providerList.innerHTML = providers.map(p => `
    <div class="settings-row">
      <span class="settings-inline-label">${escapeHtml(p.displayName)}</span>
      <button class="hook-btn" data-provider="${escapeHtml(p.id)}">${p.installed ? 'Remove' : 'Configure'}</button>
      <span class="hook-status${p.installed ? ' installed' : ''}">${p.installed ? 'Installed' : 'Not installed'}</span>
    </div>
  `).join('');

  providerList.querySelectorAll('.hook-btn[data-provider]').forEach(btn => {
    btn.onclick = async () => {
      const id = btn.dataset.provider;
      const provider = providers.find(p => p.id === id);
      if (provider?.installed) {
        await invoke('remove_provider', { id });
      } else {
        await invoke('configure_provider', { id });
      }
    };
  });
}

// --- Window resize ---

function expandedHeight() {
  let height = TITLE_HEIGHT;
  if (sessions.length === 0) {
    height += EMPTY_HEIGHT;
    // Add space for setup banner if visible
    if (needsSetup()) {
      height += 40;
    }
  } else {
    height += sessions.length * ROW_HEIGHT + 8;
    // Add space for separator if there are pinned sessions
    if (sessions.some(s => s.pinned)) {
      height += 3; // Separator height (1px) + margins (2px total)
    }
  }
  if (showSettings) {
    height += SETTINGS_HEIGHT;
  }
  return height;
}

async function applyGeometry() {
  let width, height;
  if (displayMode === 'collapsed') {
    width = collapsedWidth();
    height = COLLAPSED_HEIGHT;
  } else {
    width = EXPANDED_WIDTH;
    height = expandedHeight();
  }
  if (width === lastWidth && height === lastHeight) return;
  lastWidth = width;
  lastHeight = height;
  await invoke('set_window_size', { width, height });
}

// Re-apply geometry for the current display mode (used after in-place renders).
function resizeWindow() {
  applyGeometry();
}

// --- Settings ---

function bindToggle(elId, key, value) {
  const btn = document.getElementById(elId);
  if (!btn) return;
  btn.textContent = value ? 'On' : 'Off';
  btn.classList.toggle('on', !!value);
  btn.onclick = () => setSetting(key, (!value).toString());
}

function bindColorPicker(settingKey) {
  const picker = document.getElementById(settingKey);
  picker.value = settings[settingKey] || '#a78bfa';
  picker.oninput = () => {
    settings[settingKey] = picker.value;
    applySettings();
    render();
  };
  picker.onchange = () => setSetting(settingKey, picker.value);
}

function hexToRgb(hex) {
  const r = parseInt(hex.slice(1, 3), 16);
  const g = parseInt(hex.slice(3, 5), 16);
  const b = parseInt(hex.slice(5, 7), 16);
  return { r, g, b };
}

function applySettings() {
  document.documentElement.setAttribute('data-theme', settings.theme || 'dark');

  // State colors
  const working = settings.colorWorking || '#a78bfa';
  const waiting = settings.colorWaiting || '#fbbf24';
  const idle = settings.colorIdle || '#71717a';

  document.documentElement.style.setProperty('--color-working', working);
  document.documentElement.style.setProperty('--color-waiting', waiting);
  document.documentElement.style.setProperty('--color-idle', idle);

  // Derived color tokens (for session row backgrounds)
  const wRgb = hexToRgb(working);
  document.documentElement.style.setProperty('--accent-dim', `rgba(${wRgb.r}, ${wRgb.g}, ${wRgb.b}, 0.15)`);
  document.documentElement.style.setProperty('--accent-glow', `rgba(${wRgb.r}, ${wRgb.g}, ${wRgb.b}, 0.25)`);

  const waitRgb = hexToRgb(waiting);
  document.documentElement.style.setProperty('--waiting-dim', `rgba(${waitRgb.r}, ${waitRgb.g}, ${waitRgb.b}, 0.06)`);
  document.documentElement.style.setProperty('--waiting-glow', `rgba(${waitRgb.r}, ${waitRgb.g}, ${waitRgb.b}, 0.10)`);

  const iRgb = hexToRgb(idle);
  document.documentElement.style.setProperty('--idle-dim', `rgba(${iRgb.r}, ${iRgb.g}, ${iRgb.b}, 0.10)`);
  document.documentElement.style.setProperty('--idle-glow', `rgba(${iRgb.r}, ${iRgb.g}, ${iRgb.b}, 0.18)`);

  // Text size
  const scale = textScales[settings.textSize] || 1.0;
  document.documentElement.style.setProperty('--scale', scale);
}

async function setSetting(key, value) {
  await invoke('set_setting', { key, value });
  const boolKeys = ['soundOnComplete', 'compactMode', 'dimWhenIdle', 'clickThroughWhenIdle'];
  if (boolKeys.includes(key)) {
    settings[key] = value === 'true';
  } else {
    settings[key] = value;
  }
  applySettings();
  renderSettings();
  updateDisplay();
}

// --- Helpers ---

function formatElapsed(startTimeMs) {
  const elapsed = Math.floor((Date.now() - startTimeMs) / 1000);
  if (elapsed < 0) return '0:00';
  const mins = Math.floor(elapsed / 60);
  const secs = elapsed % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

function getDotColors() {
  const style = getComputedStyle(document.documentElement);
  return {
    idle: style.getPropertyValue('--color-idle').trim(),
    working: style.getPropertyValue('--color-working').trim(),
    waitingForUser: style.getPropertyValue('--color-waiting').trim(),
  };
}

function truncate(str, maxLen) {
  if (!str) return '';
  return str.length > maxLen ? str.substring(0, maxLen) + '\u2026' : str;
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

function startTimer() {
  timerInterval = setInterval(() => {
    sessionList.querySelectorAll('.session-row').forEach(row => {
      const session = sessions.find(s => s.id === row.dataset.id);
      if (session) {
        const timerEl = row.querySelector('.row-timer');
        if (timerEl) timerEl.textContent = formatElapsed(session.startTimeMs);
      }
    });
  }, 1000);
}

function playCompleteSound() {
  if (!settings.soundOnComplete) return;
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const now = ctx.currentTime;

    // Two-tone chime: rising interval
    const notes = [660, 880];
    notes.forEach((freq, i) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = freq;
      osc.type = 'sine';
      const start = now + i * 0.12;
      gain.gain.setValueAtTime(0.25, start);
      gain.gain.exponentialRampToValueAtTime(0.001, start + 0.4);
      osc.start(start);
      osc.stop(start + 0.4);
    });
  } catch (e) {}
}

function playWaitingSound() {
  if (!settings.soundOnComplete) return;
  try {
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const now = ctx.currentTime;

    // Three short descending notes to signal "attention needed"
    const notes = [880, 660, 880];
    notes.forEach((freq, i) => {
      const osc = ctx.createOscillator();
      const gain = ctx.createGain();
      osc.connect(gain);
      gain.connect(ctx.destination);
      osc.frequency.value = freq;
      osc.type = 'triangle';
      const start = now + i * 0.15;
      gain.gain.setValueAtTime(0.2, start);
      gain.gain.exponentialRampToValueAtTime(0.001, start + 0.25);
      osc.start(start);
      osc.stop(start + 0.25);
    });
  } catch (e) {}
}

// --- Hint Tooltips ---
const hintTooltip = document.getElementById('hintTooltip');

document.addEventListener('mouseover', (e) => {
  const icon = e.target.closest('.hint-icon');
  if (icon && icon.dataset.hint) {
    hintTooltip.textContent = icon.dataset.hint;
    hintTooltip.classList.add('visible');
  }
});

document.addEventListener('mouseout', (e) => {
  const icon = e.target.closest('.hint-icon');
  if (icon) {
    hintTooltip.classList.remove('visible');
  }
});

document.addEventListener('mousemove', (e) => {
  if (!hintTooltip.classList.contains('visible')) return;
  const pad = 12;
  const rect = hintTooltip.getBoundingClientRect();
  let x = e.clientX + pad;
  let y = e.clientY + pad;
  // Keep within window bounds
  if (x + rect.width > window.innerWidth - pad) x = e.clientX - rect.width - pad;
  if (y + rect.height > window.innerHeight - pad) y = e.clientY - rect.height - pad;
  hintTooltip.style.left = x + 'px';
  hintTooltip.style.top = y + 'px';
});

init();
