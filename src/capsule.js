const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// State
let sessions = [];
let settings = {};
let isExpanded = false;
let showSettings = false;
let hoverTimeout = null;
let leaveTimeout = null;
let timerInterval = null;
let hooksInstalled = false;

// DOM refs
const capsule = document.getElementById('capsule');
const statusDot = document.getElementById('statusDot');
const projectName = document.getElementById('projectName');
const statusText = document.getElementById('statusText');
const timer = document.getElementById('timer');
const sessionCount = document.getElementById('sessionCount');
const sessionList = document.getElementById('sessionList');
const settingsPanel = document.getElementById('settingsPanel');
const settingsBtn = document.getElementById('settingsBtn');
const soundToggle = document.getElementById('soundToggle');
const hookBtn = document.getElementById('hookBtn');
const hookStatus = document.getElementById('hookStatus');

// Color map
const accentColors = {
  purple: '#a855f7',
  cyan: '#06b6d4',
  green: '#22c55e',
  orange: '#f97316',
  pink: '#ec4899',
};

const textScales = {
  small: 0.85,
  medium: 1.0,
  large: 1.15,
};

const stateLabels = {
  idle: 'Idle',
  working: 'Working...',
  waitingForUser: 'Waiting',
  stale: 'Stale',
  stopped: 'Stopped',
};

// --- Initialization ---

async function init() {
  settings = await invoke('get_settings');
  hooksInstalled = await invoke('get_hook_status');
  sessions = await invoke('get_sessions');

  applySettings();
  render();
  startTimer();

  // Listen for backend events
  await listen('sessions-changed', (event) => {
    sessions = event.payload;
    render();
  });

  await listen('settings-changed', (event) => {
    settings = event.payload;
    applySettings();
    render();
  });

  await listen('hooks-status-changed', (event) => {
    hooksInstalled = event.payload;
    renderHookStatus();
  });

  await listen('show-settings', () => {
    showSettings = true;
    setExpanded(true);
    renderSettings();
  });

  await listen('play-sound', () => {
    playCompleteSound();
  });

  // Hover expand/collapse
  capsule.addEventListener('mouseenter', () => {
    clearTimeout(leaveTimeout);
    hoverTimeout = setTimeout(() => {
      if (sessions.length > 0) {
        setExpanded(true);
      }
    }, 200);
  });

  capsule.addEventListener('mouseleave', () => {
    clearTimeout(hoverTimeout);
    leaveTimeout = setTimeout(() => {
      if (!showSettings) {
        setExpanded(false);
      }
    }, 400);
  });

  // Settings button
  settingsBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    showSettings = !showSettings;
    if (showSettings) {
      setExpanded(true);
      renderSettings();
    } else {
      settingsPanel.style.display = 'none';
    }
  });
}

// --- Rendering ---

function render() {
  const active = sessions.find(s => s.isActive) || sessions[0];

  // Remove previous state classes
  capsule.className = 'capsule';
  if (isExpanded) capsule.classList.add('expanded');

  if (active) {
    capsule.classList.add('state-' + active.state);
    projectName.textContent = active.projectName;
    statusText.textContent = stateLabels[active.state] || active.state;
    timer.textContent = formatElapsed(active.startTimeMs);
    timer.style.display = '';
  } else {
    capsule.classList.add('state-idle');
    projectName.textContent = 'ClaudePulse';
    statusText.textContent = 'No sessions';
    timer.textContent = '';
    timer.style.display = 'none';
  }

  // Session count badge
  if (sessions.length > 1) {
    const running = sessions.filter(s => s.state === 'working' || s.state === 'waitingForUser').length;
    sessionCount.textContent = `${running}/${sessions.length}`;
    sessionCount.style.display = '';
  } else {
    sessionCount.style.display = 'none';
  }

  // Render session list
  renderSessionList();
}

function renderSessionList() {
  if (sessions.length === 0) {
    sessionList.innerHTML = '<div class="no-sessions">No active sessions.<br>Start Claude Code to see activity here.</div>';
    return;
  }

  sessionList.innerHTML = sessions.map(s => {
    const dotColor = getDotColor(s.state);
    const promptText = s.lastPrompt ? truncate(s.lastPrompt, 50) : (s.lastToolName ? `Tool: ${s.lastToolName}` : '');
    return `
      <div class="session-row ${s.isActive ? 'active' : ''}" data-id="${s.id}">
        <div class="row-dot" style="background: ${dotColor}"></div>
        <div class="row-info">
          <div class="row-project">${escapeHtml(s.projectName)}</div>
          ${promptText ? `<div class="row-prompt">${escapeHtml(promptText)}</div>` : ''}
        </div>
        <span class="row-state">${stateLabels[s.state] || s.state}</span>
        <span class="row-timer">${formatElapsed(s.startTimeMs)}</span>
      </div>
    `;
  }).join('');

  // Click handlers for session selection
  sessionList.querySelectorAll('.session-row').forEach(row => {
    row.addEventListener('click', () => {
      invoke('select_session', { id: row.dataset.id });
      // Optimistic update
      sessions.forEach(s => s.isActive = s.id === row.dataset.id);
      render();
    });
  });
}

function renderSettings() {
  settingsPanel.style.display = '';

  // Position buttons
  document.querySelectorAll('.pos-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.pos === settings.position);
    btn.onclick = () => setSetting('position', btn.dataset.pos);
  });

  // Color buttons
  document.querySelectorAll('.color-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.color === settings.accentColor);
    btn.onclick = () => setSetting('accentColor', btn.dataset.color);
  });

  // Size buttons
  document.querySelectorAll('.size-btn').forEach(btn => {
    btn.classList.toggle('active', btn.dataset.size === settings.textSize);
    btn.onclick = () => setSetting('textSize', btn.dataset.size);
  });

  // Sound toggle
  soundToggle.textContent = settings.soundOnComplete ? 'On' : 'Off';
  soundToggle.classList.toggle('on', settings.soundOnComplete);
  soundToggle.onclick = () => setSetting('soundOnComplete', (!settings.soundOnComplete).toString());

  // Hook button
  renderHookStatus();
  hookBtn.onclick = async () => {
    if (hooksInstalled) {
      await invoke('remove_hooks');
    } else {
      await invoke('configure_hooks');
    }
  };
}

function renderHookStatus() {
  hookBtn.textContent = hooksInstalled ? 'Remove Hooks' : 'Configure Hooks';
  hookStatus.textContent = hooksInstalled ? 'Installed' : 'Not installed';
  hookStatus.className = 'hook-status' + (hooksInstalled ? ' installed' : '');
}

// --- Settings ---

function applySettings() {
  // Accent color
  const color = accentColors[settings.accentColor] || accentColors.purple;
  document.documentElement.style.setProperty('--accent', color);
  document.documentElement.style.setProperty(
    '--accent-glow',
    color.replace(')', ', 0.4)').replace('rgb', 'rgba')
  );
  document.documentElement.style.setProperty('--bg-session-active',
    color.replace(')', ', 0.15)').replace('rgb', 'rgba')
  );

  // Text size
  const scale = textScales[settings.textSize] || 1.0;
  document.documentElement.style.setProperty('--scale', scale);
}

async function setSetting(key, value) {
  await invoke('set_setting', { key, value });
  // Optimistic update
  if (key === 'soundOnComplete') {
    settings[key] = value === 'true';
  } else {
    settings[key] = value;
  }
  applySettings();
  renderSettings();
}

// --- Expand/Collapse ---

async function setExpanded(expanded) {
  if (isExpanded === expanded) return;
  isExpanded = expanded;

  if (!expanded) {
    showSettings = false;
    settingsPanel.style.display = 'none';
  }

  capsule.classList.toggle('expanded', expanded);
  await invoke('set_expanded', { expanded });
}

// --- Helpers ---

function formatElapsed(startTimeMs) {
  const elapsed = Math.floor((Date.now() - startTimeMs) / 1000);
  if (elapsed < 0) return '0:00';
  const mins = Math.floor(elapsed / 60);
  const secs = elapsed % 60;
  return `${mins}:${secs.toString().padStart(2, '0')}`;
}

function getDotColor(state) {
  const colors = {
    idle: 'rgba(161, 161, 170, 0.8)',
    working: getComputedStyle(document.documentElement).getPropertyValue('--accent').trim(),
    waitingForUser: '#f59e0b',
    stale: 'rgba(113, 113, 122, 0.6)',
    stopped: 'rgba(239, 68, 68, 0.7)',
  };
  return colors[state] || colors.idle;
}

function truncate(str, maxLen) {
  if (!str) return '';
  return str.length > maxLen ? str.substring(0, maxLen) + '...' : str;
}

function escapeHtml(str) {
  const div = document.createElement('div');
  div.textContent = str;
  return div.innerHTML;
}

function startTimer() {
  timerInterval = setInterval(() => {
    const active = sessions.find(s => s.isActive) || sessions[0];
    if (active) {
      timer.textContent = formatElapsed(active.startTimeMs);
    }
    // Update session row timers
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
    // Simple beep using Web Audio API
    const ctx = new (window.AudioContext || window.webkitAudioContext)();
    const oscillator = ctx.createOscillator();
    const gain = ctx.createGain();
    oscillator.connect(gain);
    gain.connect(ctx.destination);
    oscillator.frequency.value = 880;
    oscillator.type = 'sine';
    gain.gain.setValueAtTime(0.3, ctx.currentTime);
    gain.gain.exponentialRampToValueAtTime(0.001, ctx.currentTime + 0.5);
    oscillator.start(ctx.currentTime);
    oscillator.stop(ctx.currentTime + 0.5);
  } catch (e) {
    // Silently ignore audio errors
  }
}

// Start
init();
