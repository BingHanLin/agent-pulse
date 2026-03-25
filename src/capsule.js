const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// State
let sessions = [];
let settings = {};
let showSettings = false;
let timerInterval = null;
let hooksInstalled = false;
const ROW_HEIGHT = 44;
const TITLE_HEIGHT = 36;
const EMPTY_HEIGHT = 52;
const SETTINGS_HEIGHT = 280;
let lastHeight = null;

// DOM refs
const capsule = document.getElementById('capsule');
const sessionList = document.getElementById('sessionList');
const settingsPanel = document.getElementById('settingsPanel');
const settingsBtn = document.getElementById('settingsBtn');
const soundToggle = document.getElementById('soundToggle');
const hookBtn = document.getElementById('hookBtn');
const hookStatus = document.getElementById('hookStatus');


const textScales = {
  small: 0.85,
  medium: 1.0,
  large: 1.15,
};

const stateLabels = {
  idle: 'Idle',
  working: 'Working',
  waitingForUser: 'Waiting',
};

// --- Initialization ---

async function init() {
  settings = await invoke('get_settings');
  hooksInstalled = await invoke('get_hook_status');
  sessions = await invoke('get_sessions');

  applySettings();
  render();
  startTimer();

  await listen('sessions-changed', (event) => {
    sessions = event.payload;
    render();
  });

  await listen('settings-changed', (event) => {
    settings = event.payload;
    applySettings();
    if (showSettings) renderSettings();
    render();
  });

  await listen('hooks-status-changed', (event) => {
    hooksInstalled = event.payload;
    renderHookStatus();
  });

  await listen('show-settings', () => {
    showSettings = true;
    renderSettings();
    resizeWindow();
  });

  await listen('play-sound', () => {
    playCompleteSound();
  });

  settingsBtn.addEventListener('click', (e) => {
    e.stopPropagation();
    showSettings = !showSettings;
    if (showSettings) {
      renderSettings();
    } else {
      settingsPanel.style.display = 'none';
    }
    resizeWindow();
  });
}

// --- Rendering ---

function render() {
  renderSessionList();
  resizeWindow();
}

function renderSessionList() {
  if (sessions.length === 0) {
    sessionList.innerHTML = '<div class="no-sessions">No active sessions</div>';
    return;
  }

  sessionList.innerHTML = sessions.map(s => {
    const dotColor = getDotColor(s.state);
    const stateClass = 'row-state-' + s.state;
    const promptText = s.lastPrompt ? truncate(s.lastPrompt, 40) : (s.lastToolName ? `Tool: ${s.lastToolName}` : '');
    return `
      <div class="session-row ${stateClass}" data-id="${s.id}">
        <div class="row-dot-container">
          <div class="row-dot" style="background: ${dotColor}"></div>
          <div class="row-dot-pulse" style="background: ${dotColor}"></div>
        </div>
        <div class="row-info">
          <div class="row-project">${escapeHtml(s.projectName)}<span class="debug-pid"> [${s.pid || '?'}]</span></div>
          ${promptText ? `<div class="row-prompt">${escapeHtml(promptText)}</div>` : ''}
        </div>
        <span class="row-state">${stateLabels[s.state] || s.state}</span>
        <span class="row-timer">${formatElapsed(s.startTimeMs)}</span>
      </div>
    `;
  }).join('');
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

  renderHookStatus();
  hookBtn.onclick = async () => {
    if (hooksInstalled) {
      await invoke('remove_hooks');
    } else {
      await invoke('configure_hooks');
    }
  };

  document.getElementById('resetBtn').onclick = async () => {
    await invoke('reset_settings');
  };
}

function renderHookStatus() {
  hookBtn.textContent = hooksInstalled ? 'Remove Hooks' : 'Configure Hooks';
  hookStatus.textContent = hooksInstalled ? 'Installed' : 'Not installed';
  hookStatus.className = 'hook-status' + (hooksInstalled ? ' installed' : '');
}

// --- Window resize ---

async function resizeWindow() {
  let height = TITLE_HEIGHT;
  if (sessions.length === 0) {
    height += EMPTY_HEIGHT;
  } else {
    height += sessions.length * ROW_HEIGHT + 8;
  }
  if (showSettings) {
    height += SETTINGS_HEIGHT;
  }
  if (height === lastHeight) return;
  lastHeight = height;
  await invoke('set_expanded', { height });
}

// --- Settings ---

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

  // Working color derived tokens (for session row glow)
  const { r, g, b } = hexToRgb(working);
  document.documentElement.style.setProperty('--accent-dim', `rgba(${r}, ${g}, ${b}, 0.15)`);
  document.documentElement.style.setProperty('--accent-glow', `rgba(${r}, ${g}, ${b}, 0.25)`);

  // Text size
  const scale = textScales[settings.textSize] || 1.0;
  document.documentElement.style.setProperty('--scale', scale);
}

async function setSetting(key, value) {
  await invoke('set_setting', { key, value });
  if (key === 'soundOnComplete') {
    settings[key] = value === 'true';
  } else {
    settings[key] = value;
  }
  applySettings();
  renderSettings();
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
  const style = getComputedStyle(document.documentElement);
  const colors = {
    idle: style.getPropertyValue('--color-idle').trim(),
    working: style.getPropertyValue('--color-working').trim(),
    waitingForUser: style.getPropertyValue('--color-waiting').trim(),
  };
  return colors[state] || colors.idle;
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
