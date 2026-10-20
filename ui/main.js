const { invoke, convertFileSrc } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const dialog = window.__TAURI__.dialog;

const ICONS = {
  play: '<svg viewBox="0 0 24 24" fill="currentColor"><path d="M8 5.5v13l11-6.5z"/></svg>',
  stop: '<svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="2.5"/></svg>',
  install:
    '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round"><path d="M12 4v10"/><path d="m7.5 10.5 4.5 4.5 4.5-4.5"/><path d="M5 19h14"/></svg>',
};

const IMAGE_FILTERS = [
  { name: 'Images', extensions: ['png', 'jpg', 'jpeg', 'webp', 'bmp', 'gif'] },
];

const state = {
  games: [],
  running: new Set(),
  protons: {},
  settings: {},
  tab: 'steam',
  search: '',
  showUninstalled: false,
  hideBorked: false,
  tiers: new Map(),
  sgdbCache: new Map(),
  releases: {},
  protonSource: 'ge',
  activeInstall: null,
  editingGame: null,
  gmPath: '',
  gmArtSegment: 'none',
  gmArtworkFile: null,
  artworkTarget: null,
  awSegment: 'file',
  pickedArtworkFile: null,
  deleteTarget: null,
  detailId: null,
  protonDropdown: null,
  runnerDropdown: null,
  loading: false,
};

const $ = (selector, root = document) => root.querySelector(selector);

function isTrue(value) {
  return String(value || '').toLowerCase() === 'true';
}

function basename(path) {
  return String(path || '').split('/').pop();
}

function tierKey(game) {
  return String(game.steam_id || game.id);
}

function sgdbId(game) {
  if (game.type === 'steam') return String(game.id);
  return game.steam_id ? String(game.steam_id) : null;
}

function imageSrc(artwork) {
  if (!artwork) return null;
  return artwork.startsWith('http') ? artwork : convertFileSrc(artwork);
}

function gameTier(game) {
  return state.tiers.get(tierKey(game)) || null;
}

function fmtPlaytime(seconds) {
  const s = Number(seconds) || 0;
  if (s >= 3600) {
    const hours = Math.floor(s / 3600);
    const minutes = Math.floor((s % 3600) / 60);
    return minutes > 0 ? `${hours}h ${minutes}min played` : `${hours}h played`;
  }
  if (s >= 60) return `${Math.floor(s / 60)}min played`;
  return `${s}s played`;
}

function toast(message, isError = false) {
  const element = document.createElement('div');
  element.className = 'toast' + (isError ? ' error' : '');
  element.textContent = message;
  $('#toasts').appendChild(element);
  setTimeout(() => {
    element.style.transition = 'opacity 0.3s';
    element.style.opacity = '0';
    setTimeout(() => element.remove(), 320);
  }, 3400);
}

async function mapLimit(items, limit, worker) {
  const queue = items.slice();
  const runners = Array.from({ length: Math.min(limit, queue.length) }, async () => {
    while (queue.length) {
      const item = queue.shift();
      try {
        await worker(item);
      } catch (error) {
        /* ignore individual failures */
      }
    }
  });
  await Promise.all(runners);
}

class Dropdown {
  constructor(root) {
    this.root = root;
    this.options = [];
    this.value = '';
    this.onChange = null;
    this.highlight = -1;

    const button = document.createElement('button');
    button.type = 'button';
    button.className = 'dd-btn';
    const label = document.createElement('span');
    label.className = 'dd-label';
    const caret = document.createElement('span');
    caret.className = 'dd-caret';
    caret.innerHTML =
      '<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round"><path d="m6 9 6 6 6-6"/></svg>';
    button.appendChild(label);
    button.appendChild(caret);

    const list = document.createElement('div');
    list.className = 'dd-list hidden';

    root.appendChild(button);
    root.appendChild(list);
    this.button = button;
    this.label = label;
    this.list = list;

    button.addEventListener('click', (event) => {
      event.stopPropagation();
      this.toggle();
    });
    list.addEventListener('click', (event) => event.stopPropagation());
    document.addEventListener('click', () => this.close());
    button.addEventListener('keydown', (event) => this.onKey(event));
  }

  setOptions(options, value) {
    this.options = options;
    if (typeof value === 'string') this.value = value;
    if (!this.options.some((option) => option.value === this.value)) {
      this.value = this.options.length ? this.options[0].value : '';
    }
    this.renderLabel();
    this.renderList();
  }

  renderLabel() {
    const selected = this.options.find((option) => option.value === this.value);
    this.label.textContent = selected ? selected.label : '';
  }

  renderList() {
    this.list.innerHTML = '';
    for (const option of this.options) {
      const item = document.createElement('button');
      item.type = 'button';
      item.className = 'dd-item' + (option.value === this.value ? ' selected' : '');
      item.textContent = option.label;
      item.addEventListener('click', () => this.select(option.value));
      this.list.appendChild(item);
    }
  }

  select(value) {
    this.value = value;
    this.renderLabel();
    this.renderList();
    this.close();
    this.button.focus();
    if (this.onChange) this.onChange(value);
  }

  getValue() {
    return this.value;
  }

  isOpen() {
    return !this.list.classList.contains('hidden');
  }

  toggle() {
    if (this.isOpen()) this.close();
    else this.open();
  }

  open() {
    this.list.classList.remove('hidden');
    this.root.classList.add('open');
    this.positionList();
    this.highlight = this.options.findIndex((option) => option.value === this.value);
    this.refreshHighlight();
  }

  positionList() {
    const rect = this.button.getBoundingClientRect();
    const list = this.list;
    list.style.position = 'fixed';
    list.style.left = `${rect.left}px`;
    list.style.right = 'auto';
    list.style.width = `${rect.width}px`;
    list.style.minWidth = '0';
    list.style.top = `${rect.bottom + 6}px`;
    const height = list.offsetHeight;
    if (rect.bottom + 6 + height > window.innerHeight - 8) {
      list.style.top = `${Math.max(8, rect.top - height - 6)}px`;
    }
  }

  close() {
    this.list.classList.add('hidden');
    this.root.classList.remove('open');
  }

  refreshHighlight() {
    Array.from(this.list.children).forEach((item, index) => {
      item.classList.toggle('highlight', index === this.highlight);
    });
    const active = this.list.children[this.highlight];
    if (active && active.scrollIntoView) active.scrollIntoView({ block: 'nearest' });
  }

  onKey(event) {
    const count = this.options.length;
    if (event.key === 'Escape') {
      this.close();
      return;
    }
    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault();
      if (!this.isOpen()) {
        this.open();
      } else if (this.highlight >= 0 && this.highlight < count) {
        this.select(this.options[this.highlight].value);
      }
      return;
    }
    if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
      event.preventDefault();
      const delta = event.key === 'ArrowDown' ? 1 : -1;
      if (!this.isOpen()) {
        this.open();
        return;
      }
      this.highlight = Math.max(0, Math.min(count - 1, this.highlight + delta));
      this.refreshHighlight();
    }
  }
}

function showLoading(visible) {
  $('#loading').classList.toggle('hidden', !visible);
  if (visible) $('#empty').classList.add('hidden');
}

function showModal(selector) {
  $(selector).classList.remove('hidden');
}

function hideModal(selector) {
  $(selector).classList.add('hidden');
}

/* ===== Scanning ===== */

async function scan() {
  state.loading = true;
  showLoading(true);
  try {
    const payload = await invoke('scan_library');
    state.games = payload.games || [];
    state.running = new Set(payload.running || []);
    state.protons = payload.protons || {};
    state.settings = payload.settings || {};
    state.showUninstalled = isTrue(state.settings.show_uninstalled);
    state.hideBorked = isTrue(state.settings.hide_borked);
    $('#btn-uninstalled').classList.toggle('active', state.showUninstalled);
    render();
    queueTiers();
    queueSgdb();
    if (state.detailId) refreshDetail();
  } catch (error) {
    toast(String(error), true);
  }
  state.loading = false;
  showLoading(false);
  render();
}

function visibleGames() {
  const q = (state.search || '').toLowerCase().trim();
  return state.games
    .filter((game) => game.type === state.tab)
    .filter((game) => state.showUninstalled || game.installed)
    .filter((game) => !(state.hideBorked && gameTier(game) === 'borked'))
    .filter((game) => {
      if (!q) return true;
      const name = (game.name || '').toLowerCase();
      const id = String(game.id || '').toLowerCase();
      return name.indexOf(q) !== -1 || id.indexOf(q) !== -1;
    })
    .sort((a, b) => (a.name || '').localeCompare(b.name || ''));
}

/* ===== Rendering ===== */

function render() {
  const grid = $('#grid');
  grid.innerHTML = '';
  const games = visibleGames();
  const fragment = document.createDocumentFragment();
  for (const game of games) fragment.appendChild(buildCard(game));
  grid.appendChild(fragment);
  $('#empty').classList.toggle('hidden', games.length > 0 || state.loading);
  updateCounts();
}

function updateCounts() {
  for (const type of ['steam', 'manual', 'heroic']) {
    const count = state.games.filter(
      (game) => game.type === type && (state.showUninstalled || game.installed)
    ).length;
    $(`#count-${type}`).textContent = count;
  }
}

function placeholder(game) {
  const element = document.createElement('div');
  element.className = 'cover-placeholder';
  element.textContent = (game.name || '?').trim().charAt(0) || '?';
  return element;
}

function buildCard(game) {
  const card = document.createElement('article');
  card.className = 'card';
  card.dataset.id = game.id;
  if (!game.installed) card.classList.add('uninstalled');
  if (state.running.has(game.id)) card.classList.add('running');

  const cover = document.createElement('div');
  cover.className = 'cover';

  const art = imageSrc(game.artwork);
  if (art) {
    const image = document.createElement('img');
    image.loading = 'lazy';
    image.decoding = 'async';
    image.alt = game.name;
    image.src = art;
    image.addEventListener('error', () => {
      image.remove();
      cover.prepend(placeholder(game));
    });
    cover.appendChild(image);
  } else {
    cover.appendChild(placeholder(game));
  }

  const tier = gameTier(game);
  if (tier && tier !== 'unknown') {
    const badge = document.createElement('span');
    badge.className = `badge tier ${tier}`;
    badge.textContent = tier.charAt(0).toUpperCase() + tier.slice(1);
    cover.appendChild(badge);
  }

  if (state.running.has(game.id)) {
    const badge = document.createElement('span');
    badge.className = 'badge running-badge';
    badge.textContent = 'Running';
    cover.appendChild(badge);
  } else if (!game.installed) {
    const badge = document.createElement('span');
    badge.className = 'badge installed-flag';
    badge.textContent = 'Not installed';
    cover.appendChild(badge);
  }

  const shade = document.createElement('div');
  shade.className = 'shade';
  cover.appendChild(shade);

  const name = document.createElement('div');
  name.className = 'name';
  name.textContent = game.name;
  cover.appendChild(name);

  const playtime = document.createElement('div');
  playtime.className = 'playtime';
  playtime.textContent = fmtPlaytime(game.playtime);
  cover.appendChild(playtime);

  const fab = document.createElement('button');
  fab.className = `fab${state.running.has(game.id) ? ' stop' : game.installed ? '' : ' install'}`;
  fab.innerHTML = state.running.has(game.id)
    ? ICONS.stop
    : game.installed
      ? ICONS.play
      : ICONS.install;
  fab.title = state.running.has(game.id)
    ? 'Stop'
    : game.installed
      ? 'Play'
      : 'Install via Steam';
  fab.addEventListener('click', (event) => {
    event.stopPropagation();
    playGame(game);
  });
  cover.appendChild(fab);

  card.appendChild(cover);
  card.addEventListener('click', () => openDetail(game.id));
  card.addEventListener('contextmenu', (event) => {
    event.preventDefault();
    openContextMenu(game, event.clientX, event.clientY);
  });
  return card;
}

function applyTierToCard(game) {
  const tier = gameTier(game);
  const card = $(`.card[data-id="${game.id}"]`);
  if (!card || !tier || tier === 'unknown') return;
  const cover = $('.cover', card);
  let badge = $('.badge.tier', cover);
  if (!badge) {
    badge = document.createElement('span');
    cover.appendChild(badge);
  }
  badge.className = `badge tier ${tier}`;
  badge.textContent = tier.charAt(0).toUpperCase() + tier.slice(1);
}

function applyArtworkToCard(game) {
  const card = $(`.card[data-id="${game.id}"]`);
  if (!card) return;
  const cover = $('.cover', card);
  const art = imageSrc(game.artwork);
  if (!art) return;
  let image = $('img', cover);
  if (!image) {
    const existing = $('.cover-placeholder', cover);
    if (existing) existing.remove();
    image = document.createElement('img');
    image.alt = game.name;
    cover.prepend(image);
  }
  const busted = art.includes('?') ? `${art}&t=${Date.now()}` : `${art}?t=${Date.now()}`;
  image.src = busted;
}

function updateRunningUI(gameId) {
  const running = state.running.has(gameId);
  const game = state.games.find((candidate) => candidate.id === gameId);
  const card = $(`.card[data-id="${gameId}"]`);
  if (card && game) {
    card.classList.toggle('running', running);
    const fab = $('.fab', card);
    if (fab) {
      fab.innerHTML = running ? ICONS.stop : game.installed ? ICONS.play : ICONS.install;
      fab.className = `fab${running ? ' stop' : game.installed ? '' : ' install'}`;
      fab.title = running ? 'Stop' : game.installed ? 'Play' : 'Install via Steam';
    }
    const cover = $('.cover', card);
    const runningBadge = $('.running-badge', cover);
    if (running && !runningBadge) {
      const badge = document.createElement('span');
      badge.className = 'badge running-badge';
      badge.textContent = 'Running';
      cover.appendChild(badge);
    } else if (!running && runningBadge) {
      runningBadge.remove();
    }
    const flag = $('.installed-flag', cover);
    if (flag) flag.classList.toggle('hidden', running);
  }
  if (state.detailId === gameId) refreshDetail();
}

function updatePlaytimeUI(game) {
  const card = $(`.card[data-id="${game.id}"]`);
  if (card) {
    const label = $('.playtime', card);
    if (label) label.textContent = fmtPlaytime(game.playtime);
  }
  if (state.detailId === game.id) refreshDetail();
}

/* ===== ProtonDB / SteamGridDB queues ===== */

function queueTiers() {
  const pending = [];
  const seen = new Set();
  for (const game of state.games) {
    const key = tierKey(game);
    if (state.tiers.has(key) || seen.has(key)) continue;
    seen.add(key);
    pending.push(key);
  }
  mapLimit(pending, 4, async (key) => {
    const tier = await invoke('protondb_tier', { appid: key }).catch(() => null);
    state.tiers.set(key, tier);
    if (!tier) return;
    for (const game of state.games) {
      if (tierKey(game) === key) applyTierToCard(game);
    }
    if (state.hideBorked && tier === 'borked') render();
    if (state.detailId) refreshDetail();
  });
}

function queueSgdb() {
  const key = state.settings.sgdb_api_key || '';
  if (!key.trim()) return;
  const jobs = new Map();
  for (const game of state.games) {
    if (game.artwork_source === 'user') continue;
    const id = sgdbId(game);
    if (!id || state.sgdbCache.has(id)) continue;
    if (!jobs.has(id)) jobs.set(id, []);
    jobs.get(id).push(game);
  }
  mapLimit(Array.from(jobs.entries()), 3, async ([id, games]) => {
    const url = await invoke('sgdb_cover', { gameId: id }).catch(() => null);
    state.sgdbCache.set(id, url || null);
    if (!url) return;
    for (const game of games) {
      game.artwork = url;
      game.artwork_source = 'sgdb';
      applyArtworkToCard(game);
    }
    if (state.detailId) refreshDetail();
  });
}

/* ===== Playing ===== */

async function playGame(game) {
  if (!game.installed) {
    invoke('open_external', { target: `steam://install/${game.id}` }).catch(() => {});
    return;
  }
  if (state.running.has(game.id)) {
    try {
      await invoke('stop_game', { id: game.id });
    } catch (error) {
      toast(String(error), true);
    }
    return;
  }
  toast(`Starting ${game.name}...`);
  try {
    await invoke('launch_game', { game });
  } catch (error) {
    toast(String(error), true);
  }
}

async function launchNative(game) {
  toast(`Launching ${game.name} natively...`);
  try {
    await invoke('launch_native', { id: game.id });
  } catch (error) {
    toast(String(error), true);
  }
}

async function toggleDesktopEntry(game) {
  const enable = !game.desktop_entry;
  try {
    await invoke('set_desktop_entry', { game, enabled: enable });
    game.desktop_entry = enable;
    toast(
      enable
        ? `Added ${game.name} to applications menu`
        : `Removed ${game.name} from applications menu`
    );
    if (state.detailId === game.id) refreshDetail();
  } catch (error) {
    toast(String(error), true);
  }
}

/* ===== Detail panel ===== */

function openDetail(gameId) {
  state.detailId = gameId;
  refreshDetail();
}

function refreshDetail() {
  const game = state.games.find((candidate) => candidate.id === state.detailId);
  const panel = $('#detail');
  if (!game) {
    panel.classList.add('hidden');
    return;
  }
  panel.classList.remove('hidden');

  const cover = $('#detail-cover');
  const art = imageSrc(game.artwork);
  cover.style.display = art ? 'block' : 'none';
  if (art) {
    const busted = art.includes('?') ? `${art}&t=${Date.now()}` : `${art}?t=${Date.now()}`;
    cover.src = busted;
  }

  $('#detail-name').textContent = game.name;

  const chips = $('#detail-chips');
  chips.innerHTML = '';
  const addChip = (label, className = '') => {
    const chip = document.createElement('span');
    chip.className = `chip ${className}`;
    chip.textContent = label;
    chips.appendChild(chip);
  };
  addChip(game.type === 'steam' ? 'Steam' : game.type === 'manual' ? 'Manual' : 'Heroic');
  if (game.platform) {
    addChip(game.platform === 'legendary' ? 'Epic' : game.platform === 'gog' ? 'GOG' : game.platform);
  }
  if (!game.installed) addChip('Not installed');
  if (game.type === 'manual') {
    addChip(game.runner_type === 'native' ? 'Native' : 'Proton');
    if (game.proton_version) addChip(game.proton_version);
    if (game.use_global_args) addChip('Global args');
  }
  const tier = gameTier(game);
  if (tier && tier !== 'unknown') {
    addChip(tier.charAt(0).toUpperCase() + tier.slice(1), tier);
  }

  $('#detail-playtime').textContent = fmtPlaytime(game.playtime);
  $('#detail-path').textContent = game.path || '';

  const actions = $('#detail-actions');
  actions.innerHTML = '';
  const running = state.running.has(game.id);
  const playButton = document.createElement('button');
  playButton.className = 'btn primary block';
  playButton.textContent = running ? 'Stop' : game.installed ? 'Play' : 'Install via Steam';
  playButton.addEventListener('click', () => playGame(game));
  actions.appendChild(playButton);

  const actionButton = (label, handler, className = 'btn block') => {
    const button = document.createElement('button');
    button.className = className;
    button.textContent = label;
    button.addEventListener('click', handler);
    actions.appendChild(button);
  };

  if (game.type === 'manual') {
    actionButton('Configure', () => openGameModal(game));
    actionButton('Run without Proton', () => launchNative(game));
    actionButton('Open Prefix Folder', () =>
      invoke('open_folder', { path: `~/.local/share/gamehub/prefixes/${game.id}` }).catch((error) =>
        toast(String(error), true)
      )
    );
  }
  actionButton('Change Artwork', () => openArtworkModal(game));
  if (game.installed) {
    actionButton(
      game.desktop_entry ? 'Remove from Applications Menu' : 'Add to Applications Menu',
      () => toggleDesktopEntry(game)
    );
  }
  if (game.path) {
    actionButton('Open Game Folder', () =>
      invoke('open_folder', { path: game.path }).catch((error) => toast(String(error), true))
    );
  }
  if (game.type === 'manual') {
    actionButton('Delete', () => openConfirmDelete(game), 'btn danger block');
  }
}

/* ===== Context menu ===== */

function closeContextMenu() {
  $('#ctxmenu').classList.add('hidden');
}

function openContextMenu(game, x, y) {
  const menu = $('#ctxmenu');
  menu.innerHTML = '';
  const addItem = (label, handler, danger = false) => {
    const button = document.createElement('button');
    button.textContent = label;
    if (danger) button.classList.add('danger');
    button.addEventListener('click', () => {
      closeContextMenu();
      handler();
    });
    menu.appendChild(button);
  };

  addItem('Edit Artwork', () => openArtworkModal(game));
  if (game.installed) {
    addItem(
      game.desktop_entry ? 'Remove from Applications Menu' : 'Add to Applications Menu',
      () => toggleDesktopEntry(game)
    );
  }
  addItem('Open Game Folder', () =>
    invoke('open_folder', { path: game.path || '' }).catch((error) => toast(String(error), true))
  );
  if (game.type === 'manual') {
    addItem('Configure', () => openGameModal(game));
    addItem('Open Prefix Folder', () =>
      invoke('open_folder', { path: `~/.local/share/gamehub/prefixes/${game.id}` }).catch((error) =>
        toast(String(error), true)
      )
    );
    addItem('Run without Proton', () => launchNative(game));
    menu.appendChild(document.createElement('hr'));
    addItem('Delete', () => openConfirmDelete(game), true);
  }

  menu.classList.remove('hidden');
  const rect = menu.getBoundingClientRect();
  menu.style.left = `${Math.min(x, window.innerWidth - rect.width - 8)}px`;
  menu.style.top = `${Math.min(y, window.innerHeight - rect.height - 8)}px`;
}

/* ===== Settings modal ===== */

async function saveSetting(key, value) {
  state.settings = { ...state.settings, [key]: value };
  try {
    await invoke('save_settings', { settings: state.settings });
  } catch (error) {
    toast(String(error), true);
  }
}

function populateProtonDropdown() {
  const options = [{ value: '', label: 'System Default (latest installed)' }];
  const names = Object.keys(state.protons).sort().reverse();
  for (const name of names) options.push({ value: state.protons[name], label: name });
  const current = state.settings.custom_proton_path || '';
  const value = names.some((name) => state.protons[name] === current) ? current : '';
  state.protonDropdown.setOptions(options, value);
}

function openSettings() {
  const settings = state.settings;
  $('#st-enable-api').checked = isTrue(settings.enable_steam_api);
  $('#st-hide-borked').checked = isTrue(settings.hide_borked);
  $('#st-global-args').value = settings.global_arguments || '';
  $('#st-steam-key').value = settings.steam_api_key || '';
  $('#st-sgdb-key').value = settings.sgdb_api_key || '';

  populateProtonDropdown();
  showModal('#modal-settings');
}

/* ===== Game modal (add / configure manual game) ===== */

function setGameArtSegment(segment) {
  state.gmArtSegment = segment;
  for (const button of document.querySelectorAll('#gm-art-seg button')) {
    button.classList.toggle('active', button.dataset.seg === segment);
  }
  $('#gm-art-body-none').classList.toggle('hidden', segment !== 'none');
  $('#gm-art-body-file').classList.toggle('hidden', segment !== 'file');
  $('#gm-art-body-steam').classList.toggle('hidden', segment !== 'steam');
}

function updateArgsVisibility() {
  const useGlobal = $('#gm-useglobal').checked;
  $('#gm-args-row').classList.toggle('hidden', useGlobal);
}

function openGameModal(game) {
  state.editingGame = game || null;
  state.gmPath = game ? game.path || '' : '';
  state.gmArtworkFile = null;
  $('#gm-title').textContent = game ? `Configure ${game.name}` : 'Add Manual Game';
  $('#gm-name').value = game ? game.name : '';
  $('#gm-path-label').textContent = state.gmPath ? basename(state.gmPath) : 'No file selected';
  $('#gm-dll').value = game && game.dll_overrides ? game.dll_overrides : '';

  const options = [
    { value: 'default', label: 'Global Default (from Settings)' },
    { value: 'native', label: 'Native (Linux / No Proton)' },
  ];
  const names = Object.keys(state.protons).sort().reverse();
  for (const name of names) options.push({ value: name, label: name });
  let runnerValue = 'default';
  if (game) {
    if (game.runner_type === 'native') runnerValue = 'native';
    else if (game.proton_version && names.includes(game.proton_version)) runnerValue = game.proton_version;
  }
  state.runnerDropdown.setOptions(options, runnerValue);

  $('#gm-useglobal').checked = game ? game.use_global_args !== false : true;
  $('#gm-args').value = game && game.arguments ? game.arguments : '';
  updateArgsVisibility();

  $('#gm-art-file-label').textContent = 'No image selected';
  $('#gm-art-steam').value = game && game.steam_id ? game.steam_id : '';
  $('#gm-art-search').value = '';
  setGameArtSegment(game && game.steam_id ? 'steam' : 'none');
  showModal('#modal-game');
}

async function saveGameModal() {
  const name = $('#gm-name').value.trim();
  if (!name || !state.gmPath) {
    toast('Game name and executable path are required', true);
    return;
  }
  const runner = state.runnerDropdown.getValue();
  const input = {
    name,
    path: state.gmPath,
    runner_type: runner === 'native' ? 'native' : 'proton',
    proton_version: runner !== 'default' && runner !== 'native' ? runner : null,
    use_global_args: $('#gm-useglobal').checked,
    arguments: $('#gm-args').value.trim(),
    dll_overrides: $('#gm-dll').value.trim() || null,
  };

  if (state.gmArtSegment === 'file' && state.gmArtworkFile) {
    input.artwork_type = 'file';
    input.artwork_value = state.gmArtworkFile;
  } else if (state.gmArtSegment === 'steam' && $('#gm-art-steam').value.trim()) {
    input.artwork_type = 'steam';
    input.artwork_value = $('#gm-art-steam').value.trim();
  } else if (state.editingGame) {
    input.artwork_type = 'keep';
  } else {
    input.artwork_type = 'none';
  }

  try {
    if (state.editingGame) {
      input.id = state.editingGame.id;
      await invoke('update_manual_game', { input });
    } else {
      await invoke('add_manual_game', { input });
    }
    hideModal('#modal-game');
    await scan();
  } catch (error) {
    toast(String(error), true);
  }
}

/* ===== Artwork modal ===== */

function setAwSegment(segment) {
  state.awSegment = segment;
  for (const button of document.querySelectorAll('#aw-seg button')) {
    button.classList.toggle('active', button.dataset.seg === segment);
  }
  $('#aw-body-file').classList.toggle('hidden', segment !== 'file');
  $('#aw-body-steam').classList.toggle('hidden', segment !== 'steam');
}

function openArtworkModal(game) {
  state.artworkTarget = game;
  state.pickedArtworkFile = null;
  $('#aw-file-label').textContent = 'No image selected';
  $('#aw-steam').value = game.steam_id || '';
  $('#aw-search').value = '';
  setAwSegment('file');
  showModal('#modal-artwork');
}

async function applyArtwork() {
  const game = state.artworkTarget;
  if (!game) return;
  try {
    if (state.awSegment === 'file') {
      if (!state.pickedArtworkFile) {
        toast('Choose an image file first', true);
        return;
      }
      await invoke('set_artwork', {
        id: game.id,
        kind: game.type,
        artworkType: 'file',
        artworkValue: state.pickedArtworkFile,
      });
    } else {
      const appid = $('#aw-steam').value.trim();
      if (!/^\d+$/.test(appid)) {
        toast('Enter a numeric Steam AppID', true);
        return;
      }
      await invoke('set_artwork', {
        id: game.id,
        kind: game.type,
        artworkType: 'steam',
        artworkValue: appid,
      });
    }
    hideModal('#modal-artwork');
    await scan();
  } catch (error) {
    toast(String(error), true);
  }
}

/* ===== Confirm delete ===== */

function openConfirmDelete(game) {
  state.deleteTarget = game;
  $('#cf-text').textContent = `Are you sure you want to remove '${game.name}' from your library?`;
  showModal('#modal-confirm');
}

async function confirmDelete() {
  const game = state.deleteTarget;
  if (!game) return;
  try {
    await invoke('delete_manual_game', { id: game.id });
  } catch (error) {
    toast(String(error), true);
  }
  hideModal('#modal-confirm');
  state.deleteTarget = null;
  state.detailId = state.detailId === game.id ? null : state.detailId;
  await scan();
}

/* ===== Proton manager ===== */

async function refreshProtons() {
  try {
    state.protons = await invoke('list_protons');
  } catch (error) {
    toast(String(error), true);
  }
}

function setProtonBar(fraction, indeterminate) {
  const bar = $('#pm-bar');
  bar.classList.toggle('indeterminate', indeterminate);
  if (!indeterminate) bar.style.width = `${Math.min(100, Math.max(0, fraction * 100))}%`;
}

async function loadReleases(source) {
  renderProtonList();
  try {
    state.releases[source] = await invoke('proton_releases', { source });
  } catch (error) {
    state.releases[source] = [];
    toast(String(error), true);
  }
  if (state.protonSource === source) renderProtonList();
}

function renderProtonList() {
  const list = $('#pm-list');
  list.innerHTML = '';
  const releases = state.releases[state.protonSource];
  if (!releases) {
    list.innerHTML = '<div class="muted">Checking releases...</div>';
    return;
  }
  if (!releases.length) {
    list.innerHTML = '<div class="muted">Could not fetch releases from GitHub.</div>';
    return;
  }
  const installedNames = Object.keys(state.protons);
  for (const release of releases) {
    const row = document.createElement('div');
    row.className = 'release-row';

    const tag = document.createElement('span');
    tag.className = 'tag';
    tag.textContent = release.tag;
    row.appendChild(tag);

    const right = document.createElement('span');
    right.className = 'inline';
    const isInstalled = installedNames.some(
      (name) => name.includes(release.tag) || release.tag.includes(name)
    );
    if (isInstalled) {
      const label = document.createElement('span');
      label.className = 'installed-label';
      label.textContent = 'Installed';
      right.appendChild(label);
    }
    const button = document.createElement('button');
    button.className = `btn${isInstalled ? '' : ' primary'}`;
    button.textContent = isInstalled ? 'Re-install' : 'Install';
    button.addEventListener('click', () => installProton(release));
    right.appendChild(button);
    row.appendChild(right);
    list.appendChild(row);
  }
}

async function setProtonSource(source) {
  state.protonSource = source;
  for (const button of document.querySelectorAll('#modal-proton .tabbtn')) {
    button.classList.toggle('active', button.dataset.source === source);
  }
  renderProtonList();
  if (!state.releases[source]) await loadReleases(source);
}

function openProtonManager() {
  showModal('#modal-proton');
  setProtonSource(state.protonSource);
}

async function installProton(release) {
  if (state.activeInstall) {
    toast('Another installation is already in progress', true);
    return;
  }
  state.activeInstall = release.tag;
  $('#pm-progress').classList.remove('hidden');
  $('#pm-label').textContent = `Installing ${release.tag}...`;
  setProtonBar(0, false);
  try {
    await invoke('install_proton', {
      tag: release.tag,
      url: release.url,
      checksumUrl: release.checksum_url,
    });
  } catch (error) {
    state.activeInstall = null;
    $('#pm-progress').classList.add('hidden');
    toast(String(error), true);
  }
}

/* ===== Events from the backend ===== */

function bindBackendEvents() {
  listen('game-started', (event) => {
    state.running.add(event.payload.id);
    updateRunningUI(event.payload.id);
  });
  listen('game-stopped', (event) => {
    state.running.delete(event.payload.id);
    updateRunningUI(event.payload.id);
  });
  listen('playtime-updated', (event) => {
    const { id, kind, seconds } = event.payload;
    const game = state.games.find((candidate) => candidate.id === id && candidate.type === kind);
    if (game && seconds > 0) {
      game.playtime += seconds;
      updatePlaytimeUI(game);
    }
  });
  listen('proton-progress', (event) => {
    const payload = event.payload;
    if (!state.activeInstall || payload.tag !== state.activeInstall) return;
    if (payload.status === 'downloading') {
      if (payload.total) {
        setProtonBar(payload.downloaded / payload.total, false);
        $('#pm-label').textContent = `Downloading ${payload.tag} — ${Math.round(
          (payload.downloaded / payload.total) * 100
        )}%`;
      } else {
        setProtonBar(0, true);
        $('#pm-label').textContent = `Downloading ${payload.tag}...`;
      }
    } else if (payload.status === 'extracting') {
      setProtonBar(0, true);
      $('#pm-label').textContent = `Extracting ${payload.tag}...`;
    } else if (payload.status === 'done') {
      setProtonBar(1, false);
    }
  });
  listen('proton-install-done', async (event) => {
    const payload = event.payload;
    state.activeInstall = null;
    $('#pm-progress').classList.add('hidden');
    if (payload.ok) {
      toast(`Installed ${payload.tag}`);
      await refreshProtons();
      renderProtonList();
      if (!$('#modal-settings').classList.contains('hidden')) populateProtonDropdown();
    } else {
      toast(`Install failed: ${payload.error}`, true);
    }
  });
}

/* ===== UI bindings ===== */

function bindUI() {
  state.protonDropdown = new Dropdown($('#st-proton'));
  state.protonDropdown.onChange = (value) => saveSetting('custom_proton_path', value);
  state.runnerDropdown = new Dropdown($('#gm-runner'));

  document.querySelectorAll('#tabs .tab').forEach((tab) => {
    tab.addEventListener('click', () => {
      state.tab = tab.dataset.tab;
      document.querySelectorAll('#tabs .tab').forEach((other) => other.classList.toggle('active', other === tab));
      render();
    });
  });

  $('#search').addEventListener('input', (event) => {
    state.search = event.target.value;
    render();
  });

  $('#btn-refresh').addEventListener('click', () => scan());

  $('#btn-add').addEventListener('click', () => openGameModal(null));

  $('#btn-settings').addEventListener('click', () => openSettings());

  $('#btn-uninstalled').addEventListener('click', async () => {
    state.showUninstalled = !state.showUninstalled;
    $('#btn-uninstalled').classList.toggle('active', state.showUninstalled);
    await saveSetting('show_uninstalled', state.showUninstalled ? 'True' : 'False');
    render();
  });

  $('#detail-close').addEventListener('click', () => {
    state.detailId = null;
    $('#detail').classList.add('hidden');
  });

  document.querySelectorAll('.modal-backdrop').forEach((backdrop) => {
    backdrop.addEventListener('click', (event) => {
      if (event.target === backdrop) closeBackdrop(backdrop);
    });
  });
  document.querySelectorAll('[data-close]').forEach((button) => {
    button.addEventListener('click', () => {
      const backdrop = button.closest('.modal-backdrop');
      if (backdrop) closeBackdrop(backdrop);
    });
  });

  document.addEventListener('click', closeContextMenu);
  window.addEventListener('resize', closeContextMenu);

  /* Settings */
  $('#st-enable-api').addEventListener('change', (event) =>
    saveSetting('enable_steam_api', event.target.checked ? 'True' : 'False')
  );
  $('#st-hide-borked').addEventListener('change', (event) => {
    state.hideBorked = event.target.checked;
    saveSetting('hide_borked', event.target.checked ? 'True' : 'False');
    render();
  });
  $('#st-global-args').addEventListener('change', (event) =>
    saveSetting('global_arguments', event.target.value.trim())
  );
  $('#st-steam-key').addEventListener('change', (event) =>
    saveSetting('steam_api_key', event.target.value.trim())
  );
  $('#st-sgdb-key').addEventListener('change', (event) =>
    saveSetting('sgdb_api_key', event.target.value.trim())
  );
  $('#st-steam-link').addEventListener('click', () =>
    invoke('open_external', { target: 'https://steamcommunity.com/dev/apikey' }).catch(() => {})
  );
  $('#st-sgdb-link').addEventListener('click', () =>
    invoke('open_external', { target: 'https://www.steamgriddb.com/profile/preferences' }).catch(() => {})
  );
  $('#st-proton-manage').addEventListener('click', () => openProtonManager());

  /* Game modal */
  $('#gm-browse').addEventListener('click', async () => {
    const file = await dialog
      .open({
        multiple: false,
        filters: [
          { name: 'Windows Executables', extensions: ['exe'] },
          { name: 'All Files', extensions: ['*'] },
        ],
      })
      .catch(() => null);
    if (file) {
      state.gmPath = file;
      $('#gm-path-label').textContent = basename(file);
      if (!$('#gm-name').value.trim()) {
        $('#gm-name').value = basename(file).replace(/\.exe$/i, '');
      }
    }
  });
  $('#gm-useglobal').addEventListener('change', updateArgsVisibility);
  for (const button of document.querySelectorAll('#gm-art-seg button')) {
    button.addEventListener('click', () => setGameArtSegment(button.dataset.seg));
  }
  $('#gm-art-file-btn').addEventListener('click', async () => {
    const file = await dialog.open({ multiple: false, filters: IMAGE_FILTERS }).catch(() => null);
    if (file) {
      state.gmArtworkFile = file;
      $('#gm-art-file-label').textContent = basename(file);
    }
  });
  $('#gm-art-search-btn').addEventListener('click', async () => {
    const query = $('#gm-art-search').value.trim();
    if (!query) return;
    const appid = await invoke('search_steam_appid', { name: query }).catch(() => null);
    if (appid) {
      $('#gm-art-steam').value = appid;
      toast(`Found Steam AppID ${appid}`);
    } else {
      toast('No Steam match found', true);
    }
  });
  $('#gm-save').addEventListener('click', saveGameModal);

  /* Artwork modal */
  for (const button of document.querySelectorAll('#aw-seg button')) {
    button.addEventListener('click', () => setAwSegment(button.dataset.seg));
  }
  $('#aw-file-btn').addEventListener('click', async () => {
    const file = await dialog.open({ multiple: false, filters: IMAGE_FILTERS }).catch(() => null);
    if (file) {
      state.pickedArtworkFile = file;
      $('#aw-file-label').textContent = basename(file);
    }
  });
  $('#aw-search-btn').addEventListener('click', async () => {
    const query = $('#aw-search').value.trim();
    if (!query) return;
    const appid = await invoke('search_steam_appid', { name: query }).catch(() => null);
    if (appid) {
      $('#aw-steam').value = appid;
      toast(`Found Steam AppID ${appid}`);
    } else {
      toast('No Steam match found', true);
    }
  });
  $('#aw-apply').addEventListener('click', applyArtwork);

  /* Proton manager */
  for (const button of document.querySelectorAll('#modal-proton .tabbtn')) {
    button.addEventListener('click', () => setProtonSource(button.dataset.source));
  }

  /* Confirm */
  $('#cf-yes').addEventListener('click', confirmDelete);
}

function closeBackdrop(backdrop) {
  backdrop.classList.add('hidden');
  if (backdrop.id === 'modal-settings') {
    state.hideBorked = $('#st-hide-borked').checked;
    scan();
  }
}

window.addEventListener('DOMContentLoaded', () => {
  bindUI();
  bindBackendEvents();
  scan();
});
