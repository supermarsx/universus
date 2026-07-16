//! Browser-side progressive enhancement for the server-rendered frontend.
//!
//! The HTML remains useful as a route map and accessible loading state, while
//! this small dependency-free client consumes the API gateway's real JSON
//! contracts through the same-origin `/game-api/*` bridge.

pub(crate) const CLIENT_JS: &str = r##"
(() => {
  'use strict';

  const apiPrefix = document.querySelector('meta[name="api-prefix"]')?.content || '/game-api';
  const $ = (selector, root = document) => root.querySelector(selector);
  const $$ = (selector, root = document) => Array.from(root.querySelectorAll(selector));
  const escapeHtml = (value) => String(value ?? '').replace(/[&<>"]/g, (character) => ({
    '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;'
  })[character]);
  const formatNumber = (value) => new Intl.NumberFormat().format(Number(value || 0));
  const numeric = (value) => Number.isFinite(Number(value)) ? Number(value) : 0;
  const formatDuration = (seconds) => {
    const total = Math.max(0, Number(seconds || 0));
    const hours = Math.floor(total / 3600);
    const minutes = Math.floor((total % 3600) / 60);
    const secs = Math.floor(total % 60);
    return [hours && `${hours}h`, (hours || minutes) && `${minutes}m`, `${secs}s`].filter(Boolean).join(' ');
  };
  const pretty = (value) => String(value ?? '')
    .replace(/([a-z])([A-Z])/g, '$1 $2')
    .replace(/[_-]+/g, ' ')
    .replace(/^./, (character) => character.toUpperCase());

  async function api(path, options = {}) {
    const headers = new Headers(options.headers || {});
    headers.set('Accept', 'application/json');
    if (options.body && !headers.has('Content-Type')) headers.set('Content-Type', 'application/json');

    const response = await fetch(`${apiPrefix}${path}`, { ...options, headers, credentials: 'same-origin' });
    const type = response.headers.get('content-type') || '';
    const payload = type.includes('application/json') ? await response.json() : null;
    if (!response.ok || payload?.success === false) {
      throw new Error(payload?.error || `Request failed with status ${response.status}`);
    }
    return payload && Object.prototype.hasOwnProperty.call(payload, 'data') ? payload.data : payload;
  }

  function loading(target, label) {
    target?.setAttribute('aria-busy', 'true');
    if (target) target.innerHTML = `<div class="loading-state"><span class="spinner" aria-hidden="true"></span>${escapeHtml(label)}</div>`;
  }

  function failure(target, error, retry) {
    if (!target) return;
    target.setAttribute('aria-busy', 'false');
    target.innerHTML = `<div class="error-state" role="alert"><strong>Could not load this view.</strong><span>${escapeHtml(error.message || error)}</span><button type="button" class="secondary retry-view">Try again</button></div>`;
    $('.retry-view', target)?.addEventListener('click', retry, { once: true });
  }

  function finish(target) {
    target?.setAttribute('aria-busy', 'false');
  }

  function feedback(form, message, isError = false) {
    const node = $('.form-feedback', form) || form.nextElementSibling;
    if (!node) return;
    node.textContent = message;
    node.classList.toggle('is-error', isError);
  }

  function jsonBody(value) {
    return JSON.stringify(value);
  }

  function wireAuthentication() {
    $$('[data-auth-form]').forEach((form) => {
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const submit = $('button[type="submit"]', form);
        const data = Object.fromEntries(new FormData(form).entries());
        if (form.dataset.authForm === 'register') {
          if (data.password !== data.password_confirm) {
            feedback(form, 'Passwords do not match.', true);
            return;
          }
          delete data.password_confirm;
        }
        submit.disabled = true;
        feedback(form, 'Contacting the universe gateway…');
        try {
          const payload = await api(`/api/auth/${form.dataset.authForm}`, {
            method: 'POST', body: jsonBody(data)
          });
          if (!payload?.token) throw new Error('The gateway did not return a session token.');
          feedback(form, 'Session established. Opening command center…');
          window.location.assign('/overview');
        } catch (error) {
          feedback(form, error.message, true);
          submit.disabled = false;
        }
      });
    });

    $('#logout-link')?.addEventListener('click', async (event) => {
      event.preventDefault();
      try {
        await api('/api/auth/logout', { method: 'POST', body: jsonBody({}) });
      } catch (_) {
        // Local session removal must still succeed when the gateway is offline.
      }
      window.location.assign('/login');
    });
  }

  async function loadOverview(root) {
    const reload = () => loadOverview(root);
    loading(root, 'Synchronizing command center…');
    try {
      const [planets, profile, resources] = await Promise.all([
        api('/api/planets'), api('/api/account/profile'), api('/api/account/resources')
      ]);
      const active = planets[0];
      if (!active) {
        root.innerHTML = '<div class="empty-state">No colonized planets are assigned to this account.</div>';
        return;
      }
      root.innerHTML = `
        <section class="hero-panel planet-hero">
          <img class="planet-banner" src="${escapeHtml(active.bannerUrl)}" alt="Rendered orbital view of ${escapeHtml(active.name)}">
          <div class="hero-overlay"><span class="eyebrow">Primary colony · [${formatNumber(active.galaxy)}:${formatNumber(active.system)}:${formatNumber(active.position)}]</span><h2>${escapeHtml(active.name)}</h2><p>Visual seed ${escapeHtml(active.visualSeed)} · ${escapeHtml(active.visualVersion)}</p></div>
        </section>
        <section class="metric-grid" aria-label="Resource reserves">
          ${[['Metal', resources.metal], ['Crystal', resources.crystal], ['Deuterium', resources.deuterium], ['Dark matter', resources.darkMatter]].map(([label, value]) => `<article class="metric-card"><span>${label}</span><strong>${formatNumber(value)}</strong></article>`).join('')}
        </section>
        <section class="dashboard-grid">
          <article class="panel"><div class="panel-heading"><div><span class="eyebrow">Commander profile</span><h2>${escapeHtml(profile.username)}</h2></div><span class="rank-badge">#${formatNumber(profile.rank)}</span></div><dl class="detail-list"><div><dt>Alliance</dt><dd>${escapeHtml(profile.allianceTag || 'Unaligned')}</dd></div><div><dt>Email</dt><dd>${escapeHtml(profile.email)}</dd></div></dl></article>
          <article class="panel"><div class="panel-heading"><div><span class="eyebrow">Empire</span><h2>Colonies</h2></div><span class="count-badge">${planets.length}</span></div><div class="planet-list">${planets.map((planet) => `<a class="planet-row" href="/galaxy?galaxy=${encodeURIComponent(numeric(planet.galaxy))}&system=${encodeURIComponent(numeric(planet.system))}"><span class="planet-dot" aria-hidden="true"></span><span><strong>${escapeHtml(planet.name)}</strong><small>[${formatNumber(planet.galaxy)}:${formatNumber(planet.system)}:${formatNumber(planet.position)}]</small></span><span>${formatNumber(numeric(planet.metal) + numeric(planet.crystal) + numeric(planet.deuterium))}</span></a>`).join('')}</div></article>
        </section>`;
      $('.planet-banner', root)?.addEventListener('error', (event) => event.currentTarget.classList.add('asset-missing'));
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadBuildings(root) {
    const reload = () => loadBuildings(root);
    loading(root, 'Loading colonies and construction controls…');
    try {
      const planets = await api('/api/planets');
      root.innerHTML = `
        <section class="dashboard-grid">
          <article class="panel"><span class="eyebrow">Colonies</span><h2>Construction sites</h2><div class="planet-list">${planets.map((planet) => `<button type="button" class="planet-row planet-choice" data-planet="${escapeHtml(planet.id)}"><span class="planet-dot"></span><span><strong>${escapeHtml(planet.name)}</strong><small>[${formatNumber(planet.galaxy)}:${formatNumber(planet.system)}:${formatNumber(planet.position)}]</small></span><span>${formatNumber(planet.metal)} M</span></button>`).join('')}</div></article>
          <article class="panel"><span class="eyebrow">Build queue</span><h2>Schedule upgrade</h2><form id="building-upgrade-form" class="stacked-form"><label>Colony<select name="planetId" required>${planets.map((planet) => `<option value="${escapeHtml(planet.id)}">${escapeHtml(planet.name)}</option>`).join('')}</select></label><label>Structure<select name="buildingType" required><option value="metalMine">Metal Mine</option><option value="crystalMine">Crystal Mine</option><option value="deuteriumSynthesizer">Deuterium Synthesizer</option><option value="solarPlant">Solar Plant</option><option value="roboticsFactory">Robotics Factory</option></select></label><button type="submit">Queue next level</button><p class="form-feedback" role="status" aria-live="polite"></p></form></article>
        </section>`;
      const form = $('#building-upgrade-form', root);
      $$('.planet-choice', root).forEach((button) => button.addEventListener('click', () => { form.elements.planetId.value = button.dataset.planet; form.elements.buildingType.focus(); }));
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const data = Object.fromEntries(new FormData(form));
        feedback(form, 'Scheduling construction…');
        try {
          const result = await api(`/api/planets/${encodeURIComponent(data.planetId)}/build`, { method: 'POST', body: jsonBody({ buildingType: data.buildingType }) });
          feedback(form, `${pretty(result.buildingType)} level ${result.levelTarget} queued · ${formatDuration(result.finishesInSeconds)} remaining.`);
        } catch (error) { feedback(form, error.message, true); }
      });
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadResearch(root) {
    const reload = () => loadResearch(root);
    loading(root, 'Resolving research network…');
    try {
      const [levels, queue] = await Promise.all([api('/api/research'), api('/api/research/queue')]);
      root.innerHTML = `<section class="dashboard-grid wide-first"><article class="panel"><div class="panel-heading"><div><span class="eyebrow">Technology matrix</span><h2>Research levels</h2></div></div><div class="card-grid" id="research-cards">${levels.map((tech) => `<article class="tech-card"><span class="level-chip">Lv ${formatNumber(tech.level)}</span><h3>${escapeHtml(tech.name)}</h3><p>${escapeHtml(tech.techId)}</p><div class="button-row"><button type="button" class="secondary research-cost" data-tech="${escapeHtml(tech.techId)}">Inspect cost</button><button type="button" class="research-start" data-tech="${escapeHtml(tech.techId)}">Research next level</button></div><div class="inline-result" aria-live="polite"></div></article>`).join('')}</div></article><aside class="panel"><span class="eyebrow">Active queue</span><h2>In progress</h2>${queue.length ? queue.map((item) => `<div class="queue-item"><strong>${escapeHtml(pretty(item.techId))} → ${formatNumber(item.levelTarget)}</strong><span>${formatDuration(item.finishesInSeconds)}</span><progress max="${Math.max(numeric(item.finishesInSeconds), 1)}" value="1"></progress></div>`).join('') : '<div class="empty-state compact">Research queue is idle.</div>'}</aside></section>`;
      $$('.research-cost', root).forEach((button) => button.addEventListener('click', async () => {
        const result = button.closest('.tech-card').querySelector('.inline-result');
        result.textContent = 'Calculating…';
        try { const cost = await api(`/api/research/${encodeURIComponent(button.dataset.tech)}/cost`, { method: 'POST' }); result.textContent = `Lab ${cost.planetId} · Lv ${cost.nextLevel}: ${formatNumber(cost.metal)} metal · ${formatNumber(cost.crystal)} crystal · ${formatNumber(cost.deuterium)} deuterium · ${formatDuration(cost.timeSeconds)}`; } catch (error) { result.textContent = error.message; }
      }));
      $$('.research-start', root).forEach((button) => button.addEventListener('click', async () => {
        const result = button.closest('.tech-card').querySelector('.inline-result');
        button.disabled = true;
        try { const queued = await api('/api/research/start', { method: 'POST', body: jsonBody({ technologyType: button.dataset.tech }) }); result.textContent = `Level ${queued.levelTarget} queued at lab ${queued.planetId} · ${formatDuration(queued.finishesInSeconds)}.`; } catch (error) { result.textContent = error.message; } finally { button.disabled = false; }
      }));
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadShipyard(root) {
    const reload = () => loadShipyard(root);
    loading(root, 'Opening orbital shipyard…');
    try {
      const planets = await api('/api/planets');
      const planet = planets[0];
      if (!planet) throw new Error('A colony is required before opening the shipyard.');
      const [options, queue] = await Promise.all([api(`/api/shipyard/${encodeURIComponent(planet.id)}/build-options`), api(`/api/shipyard/${encodeURIComponent(planet.id)}/queue`)]);
      root.innerHTML = `<section class="dashboard-grid"><article class="panel"><span class="eyebrow">${escapeHtml(planet.name)} orbital yard</span><h2>Production catalogue</h2><div class="card-grid">${options.map((option) => `<article class="tech-card"><h3>${escapeHtml(pretty(option.shipType))}</h3><dl class="mini-cost"><div><dt>Metal</dt><dd>${formatNumber(option.metal)}</dd></div><div><dt>Crystal</dt><dd>${formatNumber(option.crystal)}</dd></div><div><dt>Deuterium</dt><dd>${formatNumber(option.deuterium)}</dd></div></dl><small>${formatDuration(option.buildTimeSeconds)} each</small></article>`).join('')}</div><form id="shipyard-form" class="inline-form"><label>Hull<select name="shipType">${options.map((option) => `<option value="${escapeHtml(option.shipType)}">${escapeHtml(pretty(option.shipType))}</option>`).join('')}</select></label><label>Quantity<input type="number" name="quantity" min="1" max="999" value="1" required></label><button type="button" class="secondary" id="preview-build">Preview</button><button type="submit">Add to queue</button><p class="form-feedback" role="status" aria-live="polite"></p></form></article><aside class="panel"><span class="eyebrow">Assembly queue</span><h2>Active orders</h2>${queue.length ? queue.map((item) => `<div class="queue-item"><strong>${formatNumber(item.count)} × ${escapeHtml(pretty(item.shipType))}</strong><span>${formatDuration(item.completesInSeconds)}</span></div>`).join('') : '<div class="empty-state compact">No hulls are under construction.</div>'}</aside></section>`;
      const form = $('#shipyard-form', root);
      $('#preview-build', form).addEventListener('click', async () => {
        const data = Object.fromEntries(new FormData(form));
        try { const preview = await api(`/api/shipyard/${encodeURIComponent(planet.id)}/build-preview`, { method: 'POST', body: jsonBody({ ship_type: data.shipType, count: Number(data.quantity) }) }); feedback(form, `Total: ${formatNumber(preview.totalMetal)} metal · ${formatNumber(preview.totalCrystal)} crystal · ${formatDuration(preview.totalBuildTimeSeconds)}.`); } catch (error) { feedback(form, error.message, true); }
      });
      form.addEventListener('submit', async (event) => {
        event.preventDefault();
        const data = Object.fromEntries(new FormData(form));
        try { const result = await api('/api/shipyard/build', { method: 'POST', body: jsonBody({ planetId: planet.id, shipType: data.shipType, quantity: Number(data.quantity) }) }); feedback(form, `${result.quantity} × ${pretty(result.shipType)} queued · ${formatDuration(result.completesInSeconds)}.`); } catch (error) { feedback(form, error.message, true); }
      });
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadFleet(root) {
    const reload = () => loadFleet(root);
    loading(root, 'Tracking fleet telemetry…');
    try {
      const fleets = await api('/api/fleet');
      root.innerHTML = `<section class="dashboard-grid wide-first"><article class="panel"><div class="panel-heading"><div><span class="eyebrow">Live command telemetry</span><h2>Fleet movements</h2></div><span class="count-badge">${fleets.length}</span></div><div class="movement-list">${fleets.length ? fleets.map((fleet) => `<button type="button" class="movement-row" data-fleet="${escapeHtml(fleet.fleetId)}"><span class="mission-icon">${escapeHtml(String(fleet.mission).slice(0, 1).toUpperCase())}</span><span><strong>${escapeHtml(pretty(fleet.mission))}</strong><small>${escapeHtml(fleet.origin)} → ${escapeHtml(fleet.destination)}</small></span><span><strong>${formatNumber(fleet.ships)} ships</strong><small>${formatDuration(fleet.etaSeconds)}</small></span></button>`).join('') : '<div class="empty-state">No fleets are currently in transit.</div>'}</div><div id="fleet-detail" class="inline-result" aria-live="polite">Select a movement for its ship manifest.</div></article><aside class="panel"><span class="eyebrow">Command uplink</span><h2>Dispatch fleet</h2><form id="fleet-form" class="stacked-form"><label>Mission<select name="mission"><option value="transport">Transport</option><option value="attack">Attack</option><option value="deploy">Deploy</option><option value="harvest">Harvest</option></select></label><label>Target coordinates<input name="target" value="[1:121:4]" pattern="\[[0-9]+:[0-9]+:[0-9]+\]" required></label><label>Ship class<select name="shipType"><option value="smallCargo">Small Cargo</option><option value="lightFighter">Light Fighter</option><option value="cruiser">Cruiser</option></select></label><label>Count<input type="number" name="count" min="1" value="1" required></label><button type="submit">Transmit orders</button><p class="form-feedback" role="status" aria-live="polite"></p></form></aside></section>`;
      $$('.movement-row', root).forEach((button) => button.addEventListener('click', async () => {
        const detail = $('#fleet-detail', root); detail.textContent = 'Decrypting manifest…';
        try { const fleet = await api(`/api/fleet/${encodeURIComponent(button.dataset.fleet)}`); detail.innerHTML = `<strong>${escapeHtml(fleet.fleetId)} · ${escapeHtml(pretty(fleet.status))}</strong><span>${fleet.ships.map((ship) => `${formatNumber(ship.count)} × ${escapeHtml(pretty(ship.shipType))}`).join(' · ')}</span>`; } catch (error) { detail.textContent = error.message; }
      }));
      const form = $('#fleet-form', root);
      form.addEventListener('submit', async (event) => {
        event.preventDefault(); const data = Object.fromEntries(new FormData(form));
        try { const result = await api('/api/fleet/send', { method: 'POST', body: jsonBody({ mission: data.mission, target: data.target, ships: [{ shipType: data.shipType, count: Number(data.count) }] }) }); feedback(form, `${result.totalShips} ships accepted under command ${result.commandId}.`); } catch (error) { feedback(form, error.message, true); }
      });
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadGalaxy(root) {
    const controls = $('#galaxy-controls', root);
    const grid = $('#galaxy-slot-thumbnails', root);
    const status = $('#galaxy-status', root);
    const loadSystem = async () => {
      const galaxy = Number(controls.elements.galaxy.value);
      const system = Number(controls.elements.system.value);
      loading(grid, `Scanning [${galaxy}:${system}]…`);
      try {
        const view = await api(`/api/galaxy/${galaxy}/${system}`);
        const byPosition = new Map(view.slots.map((slot) => [slot.position, slot]));
        grid.innerHTML = Array.from({ length: 15 }, (_, index) => {
          const position = index + 1; const slot = byPosition.get(position);
          if (!slot) return `<article class="galaxy-slot empty"><span class="slot-number">${position}</span><div class="planet-orbit vacant"></div><strong>Unoccupied</strong><small>Available colony slot</small></article>`;
          const flags = [slot.isInactive && 'inactive', slot.isVacation && 'vacation', slot.isBanned && 'banned'].filter(Boolean).join(' · ');
          return `<article class="galaxy-slot"><span class="slot-number">${position}</span><div class="planet-orbit occupied"></div><strong>${escapeHtml(slot.planetName || slot.occupant)}</strong><small>${escapeHtml(slot.allianceTag ? `[${slot.allianceTag}] ${slot.occupant}` : slot.occupant)}</small><span class="slot-status">${escapeHtml(flags || slot.status)}</span>${slot.debrisMetal || slot.debrisCrystal ? `<small>Debris ${formatNumber(slot.debrisMetal)} M / ${formatNumber(slot.debrisCrystal)} C</small>` : ''}</article>`;
        }).join('');
        status.textContent = `System [${view.galaxy}:${view.system}] · ${view.slots.length} occupied positions`;
        finish(grid);
      } catch (error) { failure(grid, error, loadSystem); }
    };
    const params = new URLSearchParams(window.location.search);
    if (params.has('galaxy')) controls.elements.galaxy.value = params.get('galaxy');
    if (params.has('system')) controls.elements.system.value = params.get('system');
    controls.addEventListener('submit', (event) => { event.preventDefault(); loadSystem(); });
    $('#galaxy-prev', root).addEventListener('click', () => { controls.elements.system.value = Math.max(1, Number(controls.elements.system.value) - 1); loadSystem(); });
    $('#galaxy-next', root).addEventListener('click', () => { controls.elements.system.value = Math.min(499, Number(controls.elements.system.value) + 1); loadSystem(); });
    loadSystem();
  }

  async function loadLeaderboard(root) {
    const table = $('#leaderboard-table', root);
    const loadScope = async (scope) => {
      loading(table, `Loading ${scope} standings…`);
      try { const board = await api(`/api/leaderboard/${scope}`); table.innerHTML = `<table><caption>${escapeHtml(pretty(board.scope))} rankings</caption><thead><tr><th>Rank</th><th>Name</th><th class="number">Points</th></tr></thead><tbody>${board.entries.map((entry) => `<tr><td><span class="rank-badge">#${formatNumber(entry.rank)}</span></td><td><strong>${escapeHtml(entry.name)}</strong></td><td class="number">${formatNumber(entry.points)}</td></tr>`).join('')}</tbody></table>`; finish(table); } catch (error) { failure(table, error, () => loadScope(scope)); }
    };
    $$('.tab', root).forEach((button) => button.addEventListener('click', () => { $$('.tab', root).forEach((item) => { item.classList.toggle('active', item === button); item.setAttribute('aria-selected', item === button ? 'true' : 'false'); }); loadScope(button.dataset.scope); }));
    loadScope('players');
  }

  async function loadMessages(root) {
    const list = $('#message-list', root); const detail = $('#message-detail', root);
    const reload = () => loadMessages(root);
    loading(list, 'Receiving encrypted messages…');
    try {
      const messages = await api('/api/messages');
      list.innerHTML = messages.length ? messages.map((message) => `<button type="button" class="message-row ${message.unread ? 'unread' : ''}" data-message="${escapeHtml(message.id)}"><span class="unread-dot"></span><span><strong>${escapeHtml(message.subject)}</strong><small>${escapeHtml(message.from)}</small></span><time>${new Date(message.sentAt).toLocaleString()}</time></button>`).join('') : '<div class="empty-state">Inbox zero achieved. No messages are waiting.</div>';
      $$('.message-row', list).forEach((button) => button.addEventListener('click', async () => { loading(detail, 'Decrypting message…'); try { const message = await api(`/api/messages/${encodeURIComponent(button.dataset.message)}`); detail.innerHTML = `<article class="message-detail"><span class="eyebrow">From ${escapeHtml(message.from)}</span><h2>${escapeHtml(message.subject)}</h2><p>${escapeHtml(message.body)}</p></article>`; finish(detail); button.classList.remove('unread'); } catch (error) { failure(detail, error, () => button.click()); } }));
      finish(list);
    } catch (error) { failure(list, error, reload); }
    const form = $('#compose-form', root);
    form.addEventListener('submit', async (event) => { event.preventDefault(); const data = Object.fromEntries(new FormData(form)); try { const result = await api('/api/messages/send', { method: 'POST', body: jsonBody(data) }); feedback(form, `Message queued as ${result.messageId}.`); form.reset(); } catch (error) { feedback(form, error.message, true); } });
  }

  async function loadShop(root) {
    const reload = () => loadShop(root);
    loading(root, 'Loading verified marketplace catalogue…');
    try {
      const [offers, packages, resources] = await Promise.all([api('/api/shop/offers'), api('/api/shop/packages'), api('/api/account/resources')]);
      root.innerHTML = `<section class="metric-grid compact-metrics"><article class="metric-card"><span>Available dark matter</span><strong>${formatNumber(resources.darkMatter)}</strong></article><article class="metric-card"><span>Officer offers</span><strong>${offers.length}</strong></article><article class="metric-card"><span>Resource packages</span><strong>${packages.length}</strong></article></section><section class="panel"><span class="eyebrow">Officer contracts</span><h2>Temporary command staff</h2><div class="card-grid">${offers.map((offer) => `<article class="shop-card"><span class="rarity">Contract</span><h3>${escapeHtml(offer.item)}</h3><strong>${formatNumber(offer.priceDarkMatter)} DM</strong></article>`).join('')}</div></section><section class="panel"><span class="eyebrow">Purchase planner</span><h2>Resource packages</h2><div class="card-grid">${packages.map((item) => `<article class="shop-card"><h3>${escapeHtml(pretty(item.packageId))}</h3><dl class="mini-cost"><div><dt>Metal</dt><dd>${formatNumber(item.resources.metal)}</dd></div><div><dt>Crystal</dt><dd>${formatNumber(item.resources.crystal)}</dd></div><div><dt>Deuterium</dt><dd>${formatNumber(item.resources.deuterium)}</dd></div></dl><label>Quantity<input type="number" min="1" value="1" class="package-quantity"></label><button type="button" class="preview-package" data-package="${escapeHtml(item.packageId)}">Preview ${formatNumber(item.priceDarkMatter)} DM</button><p class="inline-result" aria-live="polite"></p></article>`).join('')}</div><p class="contract-note">The current gateway exposes purchase preview only. No charge is presented until a transaction endpoint exists.</p></section>`;
      $$('.preview-package', root).forEach((button) => button.addEventListener('click', async () => { const card = button.closest('.shop-card'); const result = $('.inline-result', card); try { const preview = await api('/api/shop/purchase-preview', { method: 'POST', body: jsonBody({ package_id: button.dataset.package, quantity: Number($('.package-quantity', card).value) }) }); result.textContent = `Preview total: ${formatNumber(preview.totalDarkMatter)} dark matter.`; } catch (error) { result.textContent = error.message; } }));
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadMatrixShop(root) {
    const reload = () => loadMatrixShop(root);
    loading(root, 'Decoding Matrix catalogue…');
    try {
      const [cosmetics, promotions, sales, progress] = await Promise.all([api('/api/shop-enhanced/cosmetics'), api('/api/shop-enhanced/promotions'), api('/api/shop-enhanced/flash-sales'), api('/api/shop-enhanced/matrix/progress')]);
      root.innerHTML = `<section class="hero-panel matrix-hero"><span class="eyebrow">Matrix clearance level ${formatNumber(progress.level)}</span><h2>${formatNumber(progress.points)} / ${formatNumber(progress.nextLevelAt)} points</h2><progress max="${numeric(progress.nextLevelAt)}" value="${numeric(progress.points)}"></progress></section><section class="panel"><div class="panel-heading"><h2>Cosmetic signal vault</h2><span class="count-badge">${cosmetics.length}</span></div><div class="card-grid">${cosmetics.map((item) => `<article class="shop-card ${item.matrixOnly ? 'matrix-only' : ''}"><span class="rarity">${escapeHtml(item.rarity)}</span><h3>${escapeHtml(item.name)}</h3><small>${item.matrixOnly ? 'Matrix clearance required' : 'Standard catalogue'}</small></article>`).join('')}</div></section><section class="dashboard-grid"><article class="panel"><h2>Active promotions</h2>${promotions.map((promo) => `<p><strong>${escapeHtml(promo.promoCode)}</strong> · ${formatNumber(promo.discountPercent)}%</p>`).join('') || '<div class="empty-state compact">No promotions are active.</div>'}</article><article class="panel"><h2>Flash signals</h2>${sales.map((sale) => `<p><strong>${formatNumber(sale.discountPercent)}% reduction</strong> · ${formatDuration(sale.endsInSeconds)} remaining</p>`).join('') || '<div class="empty-state compact">No flash sales are active.</div>'}</article></section>`;
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadNotifications(root) {
    const list = $('#notification-list', root); const reload = () => loadNotifications(root);
    loading(list, 'Checking command alerts…');
    try {
      const notifications = await api('/api/notifications?limit=50');
      list.innerHTML = notifications.length ? notifications.map((item) => `<article class="notification-row ${item.readAt || item.read ? '' : 'unread'}"><div><span class="eyebrow">${escapeHtml(item.category || 'system')} · priority ${escapeHtml(item.priority)}</span><h3>${escapeHtml(item.title)}</h3><p>${escapeHtml(item.message)}</p></div>${item.readAt || item.read ? '<span class="status-chip">Read</span>' : `<button type="button" class="secondary mark-read" data-notification="${escapeHtml(item.id)}">Mark read</button>`}</article>`).join('') : '<div class="empty-state">All clear. No notifications have been issued.</div>';
      $$('.mark-read', list).forEach((button) => button.addEventListener('click', async () => { try { await api(`/api/notifications/${button.dataset.notification}/read`, { method: 'POST' }); reload(); } catch (error) { button.textContent = error.message; } }));
      finish(list);
    } catch (error) { failure(list, error, reload); }
    $('#mark-all-read', root).addEventListener('click', async () => { try { await api('/api/notifications/read-all', { method: 'POST' }); reload(); } catch (error) { $('#notification-feedback', root).textContent = error.message; } });
  }

  async function loadAlliance(root) {
    const reload = () => loadAlliance(root);
    loading(root, 'Opening alliance command net…');
    try {
      const [alliance, members, diplomacy] = await Promise.all([api('/api/alliance'), api('/api/alliance/members'), api('/api/alliance/diplomacy')]);
      root.innerHTML = `<section class="hero-panel alliance-hero"><span class="eyebrow">Alliance rank #${formatNumber(alliance.rank)}</span><h2>[${escapeHtml(alliance.tag)}] ${escapeHtml(alliance.name)}</h2><p>${formatNumber(alliance.memberCount)} active members</p></section><section class="dashboard-grid"><article class="panel"><div class="panel-heading"><h2>Roster</h2><span class="count-badge">${members.length}</span></div><table><thead><tr><th>Commander</th><th>Role</th><th class="number">Points</th></tr></thead><tbody>${members.map((member) => `<tr><td>${escapeHtml(member.username)}</td><td>${escapeHtml(pretty(member.role))}</td><td class="number">${formatNumber(member.points)}</td></tr>`).join('')}</tbody></table></article><article class="panel"><h2>Diplomatic relations</h2><div class="relation-list">${diplomacy.map((relation) => { const relationClass = relation.relation === 'ally' ? 'ally' : relation.relation === 'war' ? 'war' : ''; return `<div class="relation-row"><strong>[${escapeHtml(relation.allyTag)}]</strong><span class="status-chip ${relationClass}">${escapeHtml(pretty(relation.relation))}</span></div>`; }).join('')}</div></article></section>`;
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadAccount(root) {
    const reload = () => loadAccount(root);
    loading(root, 'Loading account profile…');
    try {
      const profile = await api('/api/account/profile');
      root.innerHTML = `<section class="dashboard-grid"><article class="panel"><span class="eyebrow">Identity</span><h2>${escapeHtml(profile.username)}</h2><dl class="detail-list"><div><dt>Account ID</dt><dd>${escapeHtml(profile.id)}</dd></div><div><dt>Email</dt><dd>${escapeHtml(profile.email)}</dd></div><div><dt>Alliance</dt><dd>${escapeHtml(profile.allianceTag || 'Unaligned')}</dd></div><div><dt>Rank</dt><dd>#${formatNumber(profile.rank)}</dd></div></dl></article><article class="panel"><span class="eyebrow">Account controls</span><h2>Security and privacy</h2><div class="action-list"><a href="/account/security">Review session security <span>→</span></a><a href="/account/2fa">Configure two-factor authentication <span>→</span></a><a href="/account/privacy">Privacy and data controls <span>→</span></a><a href="/account/transfer">Account transfer <span>→</span></a></div><p class="contract-note">Profile editing is read-only until the gateway publishes an account update contract.</p></article></section>`;
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  const loaders = {
    overview: loadOverview,
    buildings: loadBuildings,
    research: loadResearch,
    shipyard: loadShipyard,
    fleet: loadFleet,
    galaxy: loadGalaxy,
    leaderboard: loadLeaderboard,
    messages: loadMessages,
    shop: loadShop,
    'matrix-shop': loadMatrixShop,
    notifications: loadNotifications,
    alliance: loadAlliance,
    account: loadAccount,
  };

  function start() {
    wireAuthentication();
    const view = $('[data-view]');
    if (!view) return;
    const loader = loaders[view.dataset.view];
    if (loader) loader(view);
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', start, { once: true });
  else start();
})();
"##;
