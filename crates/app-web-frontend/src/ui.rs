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
      const error = new Error(payload?.error || `Request failed with status ${response.status}`);
      error.status = response.status;
      error.code = payload?.code || 'request_failed';
      throw error;
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

  const SHIP_CATALOG = [
    ['smallCargo', 'Small Cargo'], ['largeCargo', 'Large Cargo'],
    ['lightFighter', 'Light Fighter'], ['heavyFighter', 'Heavy Fighter'],
    ['cruiser', 'Cruiser'], ['battleship', 'Battleship'],
    ['battlecruiser', 'Battlecruiser'], ['bomber', 'Bomber'],
    ['destroyer', 'Destroyer'], ['deathstar', 'Deathstar'],
    ['recycler', 'Recycler'], ['espionageProbe', 'Espionage Probe'],
    ['solarSatellite', 'Solar Satellite'], ['colonyShip', 'Colony Ship']
  ].map(([shipType, name]) => ({ shipType, name }));

  const safeAssetPath = (value) => {
    const path = String(value || '');
    return /^\/assets\/[A-Za-z0-9_./-]+$/.test(path) && !path.includes('..') ? path : '';
  };
  const signed = (value) => `${numeric(value) >= 0 ? '+' : '−'}${formatNumber(Math.abs(numeric(value)))}`;

  async function captureApi(path, options = {}) {
    try { return { ok: true, value: await api(path, options) }; }
    catch (error) { return { ok: false, error: error.message || String(error) }; }
  }

  async function withButtonLock(button, task) {
    if (!button || button.dataset.pending === 'true') return;
    button.dataset.pending = 'true';
    button.disabled = true;
    button.setAttribute('aria-disabled', 'true');
    try { await task(); }
    finally {
      button.dataset.pending = 'false';
      button.disabled = false;
      button.removeAttribute('aria-disabled');
    }
  }

  function clearLiveTimers(root) {
    if (root._queueTimer) window.clearInterval(root._queueTimer);
    if (root._resourceTimer) window.clearInterval(root._resourceTimer);
    root._queueTimer = null;
    root._resourceTimer = null;
  }

  function wireCountdowns(root) {
    if (root._queueTimer) window.clearInterval(root._queueTimer);
    const tick = () => {
      $$('[data-countdown]', root).forEach((node) => {
        const remaining = Math.max(0, numeric(node.dataset.remaining));
        node.textContent = remaining > 0 ? formatDuration(remaining) : 'Due for processing';
        node.dataset.remaining = String(Math.max(0, remaining - 1));
      });
    };
    tick();
    root._queueTimer = window.setInterval(() => {
      if (!root.isConnected) return window.clearInterval(root._queueTimer);
      tick();
    }, 1000);
  }

  function costMarkup(cost, secondsKey = 'timeSeconds') {
    return `<dl class="mini-cost economy-cost">
      <div><dt>Metal</dt><dd>${formatNumber(cost.metal ?? cost.totalMetal)}</dd></div>
      <div><dt>Crystal</dt><dd>${formatNumber(cost.crystal ?? cost.totalCrystal)}</dd></div>
      <div><dt>Deuterium</dt><dd>${formatNumber(cost.deuterium ?? cost.totalDeuterium)}</dd></div>
      <div><dt>Energy</dt><dd>${formatNumber(cost.energyRequired)}</dd></div>
      <div><dt>Duration</dt><dd>${formatDuration(cost[secondsKey])}</dd></div>
    </dl>`;
  }

  function planetPicker(planets, selectedId, id, label = 'Active colony') {
    return `<label for="${escapeHtml(id)}">${escapeHtml(label)}<select id="${escapeHtml(id)}" name="planetId">${planets.map((planet) => `<option value="${escapeHtml(planet.id)}" ${String(planet.id) === String(selectedId) ? 'selected' : ''}>${escapeHtml(planet.name)} [${formatNumber(planet.galaxy)}:${formatNumber(planet.system)}:${formatNumber(planet.position)}]</option>`).join('')}</select></label>`;
  }

  function resourceTelemetry(planet, projection) {
    const stocks = [
      ['Metal', planet.metal, projection.storageCap?.metal, projection.productionPerHour?.metal],
      ['Crystal', planet.crystal, projection.storageCap?.crystal, projection.productionPerHour?.crystal],
      ['Deuterium', planet.deuterium, projection.storageCap?.deuterium, projection.productionPerHour?.deuterium]
    ];
    return `<section class="resource-telemetry" aria-label="Live resources for ${escapeHtml(planet.name)}">
      <div class="resource-stock-grid">${stocks.map(([label, stock, cap, rate]) => {
        const safeCap = Math.max(1, numeric(cap));
        const percent = Math.min(100, Math.max(0, numeric(stock) / safeCap * 100));
        return `<article class="resource-stock"><span>${escapeHtml(label)}</span><strong>${formatNumber(stock)} <small>/ ${formatNumber(cap)}</small></strong><progress max="100" value="${percent}" aria-label="${escapeHtml(label)} storage ${Math.round(percent)} percent full"></progress><small class="${numeric(rate) < 0 ? 'negative' : 'positive'}">${signed(rate)} per hour</small></article>`;
      }).join('')}</div>
      <dl class="resource-breakdown">
        <div><dt>Deuterium gross</dt><dd>${signed(projection.productionBreakdown?.deuteriumGrossPerHour)} / h</dd></div>
        <div><dt>Fusion fuel burn</dt><dd class="negative">${signed(-Math.abs(numeric(projection.productionBreakdown?.fusionFuelPerHour)))} / h</dd></div>
        <div><dt>Energy supply</dt><dd>${formatNumber(projection.energy?.supply)}</dd></div>
        <div><dt>Energy demand</dt><dd>${formatNumber(projection.energy?.demand)}</dd></div>
        <div><dt>Energy net</dt><dd class="${numeric(projection.energy?.net) < 0 ? 'negative' : 'positive'}">${signed(projection.energy?.net)}</dd></div>
        <div><dt>Production factor</dt><dd>${formatNumber(numeric(projection.productionFactor) * 100)}%</dd></div>
        <div><dt>Fusion reactor</dt><dd>${projection.fusionOnline ? 'Online' : 'Offline'}</dd></div>
      </dl>
    </section>`;
  }

  function installResourcePolling(root, planetId, targetSelector) {
    if (root._resourceTimer) window.clearInterval(root._resourceTimer);
    root._resourceTimer = window.setInterval(async () => {
      if (!root.isConnected) return window.clearInterval(root._resourceTimer);
      try {
        const [planets, projection] = await Promise.all([
          api('/api/planets'),
          api(`/api/planets/${encodeURIComponent(planetId)}/resources`)
        ]);
        const planet = planets.find((item) => String(item.id) === String(planetId));
        const target = $(targetSelector, root);
        if (planet && target) target.innerHTML = resourceTelemetry(planet, projection);
      } catch (_) {
        // Keep the last confirmed snapshot; the next interval retries without
        // replacing useful telemetry with an invented state.
      }
    }, 30000);
  }

  function queueCountdown(seconds) {
    return `<time data-countdown data-remaining="${Math.max(0, numeric(seconds))}">${formatDuration(seconds)}</time>`;
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
    clearLiveTimers(root);
    loading(root, 'Synchronizing command center…');
    try {
      const [planets, profile, resources] = await Promise.all([
        api('/api/planets'), api('/api/account/profile'), api('/api/account/resources')
      ]);
      const active = planets.find((planet) => String(planet.id) === root.dataset.selectedPlanet) || planets[0];
      if (!active) {
        root.innerHTML = '<div class="empty-state">No colonized planets are assigned to this account.</div>';
        finish(root);
        return;
      }
      root.dataset.selectedPlanet = String(active.id);
      const projection = await api(`/api/planets/${encodeURIComponent(active.id)}/resources`);
      const banner = safeAssetPath(active.bannerUrl);
      root.innerHTML = `
        <div class="progression-toolbar">
          ${planetPicker(planets, active.id, 'overview-planet-select')}
          <button type="button" class="secondary" id="overview-refresh">Refresh telemetry</button>
        </div>
        <section class="hero-panel planet-hero">
          ${banner ? `<img class="planet-banner" src="${escapeHtml(banner)}" alt="Rendered orbital view of ${escapeHtml(active.name)}">` : '<div class="planet-banner asset-missing" role="img" aria-label="Planet visual unavailable"></div>'}
          <div class="hero-overlay"><span class="eyebrow">Selected colony · [${formatNumber(active.galaxy)}:${formatNumber(active.system)}:${formatNumber(active.position)}]</span><h2>${escapeHtml(active.name)}</h2><p>Visual seed ${escapeHtml(active.visualSeed)} · ${escapeHtml(active.visualVersion)}</p></div>
        </section>
        <section class="metric-grid" aria-label="Resource reserves">
          ${[['Metal', resources.metal], ['Crystal', resources.crystal], ['Deuterium', resources.deuterium], ['Dark matter', resources.darkMatter]].map(([label, value]) => `<article class="metric-card"><span>${label}</span><strong>${formatNumber(value)}</strong></article>`).join('')}
        </section>
        <div id="overview-resources">${resourceTelemetry(active, projection)}</div>
        <section class="dashboard-grid">
          <article class="panel"><div class="panel-heading"><div><span class="eyebrow">Commander profile</span><h2>${escapeHtml(profile.username)}</h2></div><span class="rank-badge">#${formatNumber(profile.rank)}</span></div><dl class="detail-list"><div><dt>Alliance</dt><dd>${escapeHtml(profile.allianceTag || 'Unaligned')}</dd></div><div><dt>Email</dt><dd>${escapeHtml(profile.email)}</dd></div></dl><form id="rename-planet-form" class="inline-form"><label for="planet-name">Rename selected colony<input id="planet-name" name="name" value="${escapeHtml(active.name)}" maxlength="100" required></label><button type="submit">Rename planet</button><p class="form-feedback" role="status" aria-live="polite"></p></form></article>
          <article class="panel"><div class="panel-heading"><div><span class="eyebrow">Empire</span><h2>Colonies</h2></div><span class="count-badge">${planets.length}</span></div><div class="planet-list">${planets.map((planet) => `<a class="planet-row" href="/galaxy?galaxy=${encodeURIComponent(numeric(planet.galaxy))}&system=${encodeURIComponent(numeric(planet.system))}"><span class="planet-dot" aria-hidden="true"></span><span><strong>${escapeHtml(planet.name)}</strong><small>[${formatNumber(planet.galaxy)}:${formatNumber(planet.system)}:${formatNumber(planet.position)}]</small></span><span>${formatNumber(numeric(planet.metal) + numeric(planet.crystal) + numeric(planet.deuterium))}</span></a>`).join('')}</div></article>
        </section>`;
      $('.planet-banner', root)?.addEventListener('error', (event) => event.currentTarget.classList.add('asset-missing'));
      $('#overview-planet-select', root)?.addEventListener('change', (event) => {
        root.dataset.selectedPlanet = event.currentTarget.value;
        reload();
      });
      $('#overview-refresh', root)?.addEventListener('click', reload);
      const renameForm = $('#rename-planet-form', root);
      renameForm?.addEventListener('submit', (event) => {
        event.preventDefault();
        const submit = $('button[type="submit"]', renameForm);
        withButtonLock(submit, async () => {
          feedback(renameForm, 'Renaming colony…');
          try {
            const result = await api(`/api/planets/${encodeURIComponent(active.id)}/rename`, {
              method: 'POST', body: jsonBody({ name: String(new FormData(renameForm).get('name') || '') })
            });
            feedback(renameForm, `${result.oldName} renamed to ${result.newName}.`);
            await reload();
          } catch (error) { feedback(renameForm, error.message, true); }
        });
      });
      installResourcePolling(root, active.id, '#overview-resources');
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadBuildings(root) {
    const reload = () => loadBuildings(root);
    clearLiveTimers(root);
    loading(root, 'Loading colonies and construction controls…');
    try {
      const planets = await api('/api/planets');
      const planet = planets.find((item) => String(item.id) === root.dataset.selectedPlanet) || planets[0];
      if (!planet) {
        root.innerHTML = '<div class="empty-state">A colonized planet is required before construction can begin.</div>';
        finish(root);
        return;
      }
      root.dataset.selectedPlanet = String(planet.id);
      const [projection, catalog, queue] = await Promise.all([
        api(`/api/planets/${encodeURIComponent(planet.id)}/resources`),
        api(`/api/planets/${encodeURIComponent(planet.id)}/buildings`),
        api(`/api/planets/${encodeURIComponent(planet.id)}/build-queue`)
      ]);
      const available = catalog.filter((building) => building.available && building.quote);
      root.innerHTML = `
        <div class="progression-toolbar">
          ${planetPicker(planets, planet.id, 'building-planet-select', 'Construction colony')}
          <button type="button" class="secondary" id="building-refresh">Refresh queue and quotes</button>
        </div>
        <div id="building-resources">${resourceTelemetry(planet, projection)}</div>
        <section class="progression-layout">
          <article class="panel progression-catalog"><div class="panel-heading"><div><span class="eyebrow">Canonical catalogue</span><h2>All structures</h2></div><span class="count-badge">${catalog.length}</span></div>
            <p class="contract-note">Quotes are current previews, not reservations. Affordability and queue availability are revalidated atomically when submitted.</p><div class="card-grid building-catalog">${catalog.map((building) => `<article class="tech-card ${building.available ? '' : 'is-locked'}"><div class="panel-heading"><h3>${escapeHtml(building.name)}</h3><span class="level-chip">Lv ${formatNumber(building.currentLevel)} → ${formatNumber(building.nextLevel)}</span></div>${building.quote ? costMarkup(building.quote) : `<p class="unavailable-reason" role="note">${escapeHtml(building.unavailableReason || 'The server did not publish an upgrade quote.')}</p>`}<span class="status-chip ${building.available ? 'available' : 'locked'}">${building.available ? 'Prerequisites satisfied' : 'Unavailable — see reason'}</span></article>`).join('')}</div>
          </article>
          <aside class="progression-sidebar">
            <section class="panel"><div class="panel-heading"><div><span class="eyebrow">Construction queue</span><h2>${escapeHtml(planet.name)}</h2></div><button type="button" class="secondary compact-button" id="building-queue-refresh">Refresh</button></div>${queue.length ? queue.map((item) => `<div class="queue-item"><strong>${escapeHtml(item.name)} → level ${formatNumber(item.levelTarget)}</strong><span>${escapeHtml(pretty(item.status))} · ${queueCountdown(item.finishesInSeconds)}</span></div>`).join('') : '<div class="empty-state compact">The construction queue is idle.</div>'}</section>
            <section class="panel"><span class="eyebrow">Exact server quote</span><h2>Schedule next level</h2>${available.length ? `<form id="building-upgrade-form" class="stacked-form"><label for="building-type">Structure<select id="building-type" name="buildingType" required>${available.map((building) => `<option value="${escapeHtml(building.buildingType)}">${escapeHtml(building.name)} · level ${formatNumber(building.nextLevel)}</option>`).join('')}</select></label><div id="building-quote" class="quote-preview" aria-live="polite"></div><button type="button" class="secondary" id="building-quote-refresh">Refresh exact quote</button><button type="submit" ${queue.length ? 'disabled' : ''}>${queue.length ? 'Construction queue occupied' : 'Queue next level'}</button><p class="form-feedback ${queue.length ? 'is-error' : ''}" role="status" aria-live="polite">${queue.length ? 'The durable repository permits one active construction order for this colony. Wait for it to complete before enqueueing another.' : ''}</p></form>` : '<div class="empty-state compact">No structure currently meets its authoritative prerequisites.</div>'}</section>
          </aside>
        </section>`;
      $('#building-planet-select', root)?.addEventListener('change', (event) => {
        root.dataset.selectedPlanet = event.currentTarget.value;
        reload();
      });
      $('#building-refresh', root)?.addEventListener('click', reload);
      $('#building-queue-refresh', root)?.addEventListener('click', reload);
      const form = $('#building-upgrade-form', root);
      const refreshQuote = async () => {
        if (!form) return null;
        const target = $('#building-quote', form);
        target.textContent = 'Refreshing authoritative quote…';
        try {
          const quote = await api(`/api/planets/${encodeURIComponent(planet.id)}/build-quote`, {
            method: 'POST', body: jsonBody({ buildingType: form.elements.buildingType.value })
          });
          target.classList.remove('is-error');
          target.innerHTML = `<strong>${escapeHtml(quote.name)} · level ${formatNumber(quote.nextLevel)}</strong>${costMarkup(quote)}`;
          return quote;
        } catch (error) {
          target.textContent = error.message;
          target.classList.add('is-error');
          return null;
        }
      };
      form?.elements.buildingType.addEventListener('change', refreshQuote);
      if (form) $('#building-quote-refresh', form)?.addEventListener('click', refreshQuote);
      form?.addEventListener('submit', (event) => {
        event.preventDefault();
        if (queue.length) return feedback(form, 'The construction queue is occupied for this colony.', true);
        const submit = $('button[type="submit"]', form);
        withButtonLock(submit, async () => {
          feedback(form, 'Validating current price and prerequisites…');
          const quote = await refreshQuote();
          if (!quote) return feedback(form, 'Construction cannot be queued without a current server quote.', true);
          try {
            const result = await api(`/api/planets/${encodeURIComponent(planet.id)}/build`, { method: 'POST', body: jsonBody({ buildingType: quote.buildingType }) });
            feedback(form, `${pretty(result.buildingType)} level ${result.levelTarget} queued · ${formatDuration(result.finishesInSeconds)} remaining.`);
            await reload();
          } catch (error) { feedback(form, error.message, true); }
        });
      });
      if (form) await refreshQuote();
      wireCountdowns(root);
      installResourcePolling(root, planet.id, '#building-resources');
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadResearch(root) {
    const reload = () => loadResearch(root);
    clearLiveTimers(root);
    loading(root, 'Resolving research network…');
    try {
      const [levels, queue] = await Promise.all([api('/api/research'), api('/api/research/queue')]);
      const quotes = await Promise.all(levels.map((tech) => captureApi(
        `/api/research/${encodeURIComponent(tech.techId)}/cost`, { method: 'POST' }
      )));
      const technologies = levels.map((tech, index) => ({ ...tech, quote: quotes[index] }));
      root.innerHTML = `<div class="progression-toolbar"><p class="contract-note"><strong>Account-global research:</strong> the gateway authoritatively selects your highest-level research lab for both quote and enqueue.</p><button type="button" class="secondary" id="research-refresh">Refresh queue and quotes</button></div><section class="progression-layout"><article class="panel progression-catalog"><div class="panel-heading"><div><span class="eyebrow">Technology matrix</span><h2>All research disciplines</h2></div><span class="count-badge">${technologies.length}</span></div><div class="card-grid" id="research-cards">${technologies.map((tech) => `<article class="tech-card ${tech.quote.ok ? '' : 'is-locked'}"><div class="panel-heading"><h3>${escapeHtml(tech.name)}</h3><span class="level-chip">Lv ${formatNumber(tech.level)} → ${formatNumber(numeric(tech.level) + 1)}</span></div>${tech.quote.ok ? `<p class="quote-context">Research lab planet ${escapeHtml(tech.quote.value.planetId)}</p>${costMarkup(tech.quote.value)}` : `<p class="unavailable-reason" role="note">${escapeHtml(tech.quote.error)}</p>`}<button type="button" class="research-start" data-tech="${escapeHtml(tech.techId)}" ${tech.quote.ok && !queue.length ? '' : 'disabled'}>${queue.length ? 'Research queue occupied' : tech.quote.ok ? 'Research next level' : 'Prerequisites unmet'}</button><div class="inline-result" role="status" aria-live="polite"></div></article>`).join('')}</div></article><aside class="progression-sidebar"><section class="panel"><div class="panel-heading"><div><span class="eyebrow">Active queue</span><h2>Research in progress</h2></div><button type="button" class="secondary compact-button" id="research-queue-refresh">Refresh</button></div>${queue.length ? queue.map((item) => `<div class="queue-item"><strong>${escapeHtml(pretty(item.techId))} → level ${formatNumber(item.levelTarget)}</strong><span>Lab planet ${escapeHtml(item.planetId)} · ${queueCountdown(item.finishesInSeconds)}</span></div>`).join('') : '<div class="empty-state compact">The research queue is idle.</div>'}</section></aside></section>`;
      $('#research-refresh', root)?.addEventListener('click', reload);
      $('#research-queue-refresh', root)?.addEventListener('click', reload);
      $$('.research-start:not(:disabled)', root).forEach((button) => button.addEventListener('click', () => {
        const result = $('.inline-result', button.closest('.tech-card'));
        withButtonLock(button, async () => {
          result.textContent = 'Revalidating authoritative research quote…';
          const quote = await captureApi(`/api/research/${encodeURIComponent(button.dataset.tech)}/cost`, { method: 'POST' });
          if (!quote.ok) {
            result.textContent = quote.error;
            result.classList.add('is-error');
            return;
          }
          try {
            const queued = await api('/api/research/start', { method: 'POST', body: jsonBody({ technologyType: button.dataset.tech }) });
            result.textContent = `Level ${queued.levelTarget} queued at lab ${queued.planetId} · ${formatDuration(queued.finishesInSeconds)}.`;
            await reload();
          } catch (error) {
            result.textContent = error.message;
            result.classList.add('is-error');
          }
        });
      }));
      wireCountdowns(root);
      finish(root);
    } catch (error) { failure(root, error, reload); }
  }

  async function loadShipyard(root) {
    const reload = () => loadShipyard(root);
    clearLiveTimers(root);
    loading(root, 'Opening orbital shipyard…');
    try {
      const planets = await api('/api/planets');
      const planet = planets.find((item) => String(item.id) === root.dataset.selectedPlanet) || planets[0];
      if (!planet) {
        root.innerHTML = '<div class="empty-state">A colonized planet is required before opening the shipyard.</div>';
        finish(root);
        return;
      }
      root.dataset.selectedPlanet = String(planet.id);
      const [projection, options, queue] = await Promise.all([
        api(`/api/planets/${encodeURIComponent(planet.id)}/resources`),
        api(`/api/shipyard/${encodeURIComponent(planet.id)}/build-options`),
        api(`/api/shipyard/${encodeURIComponent(planet.id)}/queue`)
      ]);
      const previews = await Promise.all(SHIP_CATALOG.map((ship) => captureApi(
        `/api/shipyard/${encodeURIComponent(planet.id)}/build-preview`, {
          method: 'POST', body: jsonBody({ shipType: ship.shipType, quantity: 1 })
        }
      )));
      const published = new Map(options.map((option) => [String(option.shipType), option]));
      const ships = SHIP_CATALOG.map((ship, index) => {
        const option = published.get(ship.shipType);
        const preview = previews[index];
        const unavailableReason = !preview.ok
          ? preview.error
          : !option
            ? 'The gateway did not publish this hull as buildable for the selected colony.'
            : '';
        return { ...ship, preview, prerequisitesSatisfied: Boolean(option && preview.ok), unavailableReason };
      });
      const available = ships.filter((ship) => ship.prerequisitesSatisfied);
      root.innerHTML = `<div class="progression-toolbar">${planetPicker(planets, planet.id, 'shipyard-planet-select', 'Shipyard colony')}<button type="button" class="secondary" id="shipyard-refresh">Refresh queue and catalogue</button></div><div id="shipyard-resources">${resourceTelemetry(planet, projection)}</div><section class="progression-layout"><article class="panel progression-catalog"><div class="panel-heading"><div><span class="eyebrow">${escapeHtml(planet.name)} orbital yard</span><h2>All ship classes</h2></div><span class="count-badge">${ships.length}</span></div><p class="contract-note">The completeness index supplies labels only. Every cost, duration, availability decision, and lock reason below comes from the gateway.</p><div class="card-grid ship-catalog">${ships.map((ship) => `<article class="tech-card ${ship.prerequisitesSatisfied ? '' : 'is-locked'}"><h3>${escapeHtml(ship.name)}</h3>${ship.preview.ok ? costMarkup(ship.preview.value, 'totalBuildTimeSeconds') : ''}${ship.unavailableReason ? `<p class="unavailable-reason" role="note">${escapeHtml(ship.unavailableReason)}</p>` : ''}<span class="status-chip ${ship.prerequisitesSatisfied ? 'available' : 'locked'}">${ship.prerequisitesSatisfied ? 'Prerequisites satisfied' : 'Unavailable — see reason'}</span></article>`).join('')}</div></article><aside class="progression-sidebar"><section class="panel"><div class="panel-heading"><div><span class="eyebrow">Assembly queue</span><h2>Active orders</h2></div><button type="button" class="secondary compact-button" id="shipyard-queue-refresh">Refresh</button></div>${queue.length ? queue.map((item) => `<div class="queue-item"><strong>${formatNumber(item.count)} × ${escapeHtml(pretty(item.shipType))}</strong><span>${queueCountdown(item.completesInSeconds)}</span></div>`).join('') : '<div class="empty-state compact">No hulls are under construction.</div>'}</section><section class="panel"><span class="eyebrow">Exact quantity quote</span><h2>Schedule production</h2>${available.length ? `<form id="shipyard-form" class="stacked-form"><label for="shipyard-type">Hull<select id="shipyard-type" name="shipType">${available.map((ship) => `<option value="${escapeHtml(ship.shipType)}">${escapeHtml(ship.name)}</option>`).join('')}</select></label><label for="shipyard-quantity">Quantity<input id="shipyard-quantity" type="number" name="quantity" min="1" max="1000000000" value="1" inputmode="numeric" required></label><div id="shipyard-quote" class="quote-preview" aria-live="polite"></div><button type="button" class="secondary" id="preview-build">Refresh exact quote</button><button type="submit" ${queue.length ? 'disabled' : ''}>${queue.length ? 'Shipyard queue occupied' : 'Add to queue'}</button><p class="form-feedback ${queue.length ? 'is-error' : ''}" role="status" aria-live="polite">${queue.length ? 'The durable repository permits one active shipyard order for this colony. Wait for it to complete before enqueueing another.' : ''}</p></form>` : '<div class="empty-state compact">No ship class currently meets its authoritative prerequisites.</div>'}</section></aside></section>`;
      $('#shipyard-planet-select', root)?.addEventListener('change', (event) => {
        root.dataset.selectedPlanet = event.currentTarget.value;
        reload();
      });
      $('#shipyard-refresh', root)?.addEventListener('click', reload);
      $('#shipyard-queue-refresh', root)?.addEventListener('click', reload);
      const form = $('#shipyard-form', root);
      const refreshPreview = async () => {
        if (!form) return null;
        const data = Object.fromEntries(new FormData(form));
        const target = $('#shipyard-quote', form);
        target.textContent = 'Refreshing authoritative quantity quote…';
        try {
          const preview = await api(`/api/shipyard/${encodeURIComponent(planet.id)}/build-preview`, { method: 'POST', body: jsonBody({ shipType: data.shipType, quantity: Number(data.quantity) }) });
          target.classList.remove('is-error');
          target.innerHTML = `<strong>${formatNumber(preview.count)} × ${escapeHtml(pretty(preview.shipType))}</strong>${costMarkup(preview, 'totalBuildTimeSeconds')}`;
          return preview;
        } catch (error) {
          target.textContent = error.message;
          target.classList.add('is-error');
          return null;
        }
      };
      if (form) {
        $('#preview-build', form)?.addEventListener('click', refreshPreview);
        form.elements.shipType.addEventListener('change', refreshPreview);
        form.elements.quantity.addEventListener('change', refreshPreview);
      }
      form?.addEventListener('submit', (event) => {
        event.preventDefault();
        if (queue.length) return feedback(form, 'The shipyard queue is occupied for this colony.', true);
        const submit = $('button[type="submit"]', form);
        withButtonLock(submit, async () => {
          feedback(form, 'Validating current price and prerequisites…');
          const preview = await refreshPreview();
          if (!preview) return feedback(form, 'Production cannot be queued without a current server quote.', true);
          try {
            const result = await api('/api/shipyard/build', { method: 'POST', body: jsonBody({ planetId: planet.id, shipType: preview.shipType, quantity: preview.count }) });
            feedback(form, `${result.quantity} × ${pretty(result.shipType)} queued · ${formatDuration(result.completesInSeconds)}.`);
            await reload();
          } catch (error) { feedback(form, error.message, true); }
        });
      });
      if (form) await refreshPreview();
      wireCountdowns(root);
      installResourcePolling(root, planet.id, '#shipyard-resources');
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

  const PRIVACY_CHANNELS = ['email', 'in_app', 'push', 'sms'];
  const PRIVACY_CATEGORIES = ['marketing', 'product_updates', 'gameplay_digest', 'security', 'transactional'];
  const privacyDate = (unix) => unix ? new Intl.DateTimeFormat(undefined, { dateStyle: 'medium', timeStyle: 'short' }).format(new Date(Number(unix) * 1000)) : 'Not recorded';
  const privacyStatus = (status) => `<span class="status-chip privacy-status privacy-status-${escapeHtml(status)}">${escapeHtml(pretty(status))}</span>`;
  const privacyVersionConflict = (error) => error?.code === 'privacy_version_conflict' || error?.status === 409;

  function privacyIdempotencyKey(root, requestType) {
    root._privacyRequestKeys ||= {};
    if (!root._privacyRequestKeys[requestType]) {
      const nonce = globalThis.crypto?.randomUUID?.() || `${Date.now()}-${Math.random().toString(36).slice(2)}`;
      root._privacyRequestKeys[requestType] = `self-service-${requestType}-${nonce}`;
    }
    return root._privacyRequestKeys[requestType];
  }

  function privacyRequestMarkup(request) {
    const cooling = request.coolingOffUntilUnix && Number(request.coolingOffUntilUnix) > Date.now() / 1000;
    let exportState = '';
    if (request.requestType === 'export' && request.export) {
      exportState = request.export.expired
        ? '<p class="privacy-delivery-note is-error">The prepared export expired. Create a new export request if it is still needed.</p>'
        : request.export.ready
          ? `<p class="privacy-delivery-note"><strong>Export prepared.</strong> ${formatNumber(request.export.plaintextSize)} bytes · retained until ${escapeHtml(privacyDate(request.export.expiresAtUnix))}. Secure delivery is not connected in this app, so no download is offered.</p>`
          : '<p class="privacy-delivery-note">The export is still being prepared.</p>';
    }
    return `<article class="privacy-request-card" data-request-id="${escapeHtml(request.id)}">
      <div class="panel-heading"><div><span class="eyebrow">${escapeHtml(pretty(request.requestType))} request #${escapeHtml(request.id)}</span><h3>${escapeHtml(privacyDate(request.requestedAtUnix))}</h3></div>${privacyStatus(request.status)}</div>
      <div class="privacy-badges">${request.legalHoldActive ? '<span class="status-chip war">Legal hold active</span>' : ''}${cooling ? `<span class="status-chip">Cooling off until ${escapeHtml(privacyDate(request.coolingOffUntilUnix))}</span>` : ''}<span class="status-chip">Version ${formatNumber(request.version)}</span></div>
      ${exportState}
      <button type="button" class="secondary privacy-detail-button" data-request-detail="${escapeHtml(request.id)}">View timeline and controls</button>
    </article>`;
  }

  function privacyTimelineMarkup(detail) {
    const request = detail.request;
    const timeline = detail.timeline || [];
    const cancelForm = request.cancellationAllowed ? `<form class="stacked-form privacy-cancel-form" data-request-id="${escapeHtml(request.id)}" data-request-version="${escapeHtml(request.version)}">
      <label>Type <code>CANCEL REQUEST</code> to cancel this request
        <input name="confirmation" autocomplete="off" required aria-describedby="privacy-cancel-help-${escapeHtml(request.id)}">
      </label>
      <small id="privacy-cancel-help-${escapeHtml(request.id)}">Cancellation is recorded in the immutable request timeline. Work that has already completed cannot be cancelled.</small>
      <button type="submit" class="secondary">Cancel request</button>
      <p class="form-feedback" role="status" aria-live="polite"></p>
    </form>` : '<p class="contract-note">This request can no longer be cancelled from self-service.</p>';
    return `<div class="panel-heading"><div><span class="eyebrow">Request #${escapeHtml(request.id)}</span><h2>${escapeHtml(pretty(request.requestType))} timeline</h2></div>${privacyStatus(request.status)}</div>
      ${request.legalHoldActive ? '<div class="notice-card privacy-hold" role="status"><strong>Legal hold active.</strong> Processing and cancellation are paused while the hold remains in force.</div>' : ''}
      ${request.coolingOffUntilUnix ? `<p class="contract-note">Cooling-off boundary: ${escapeHtml(privacyDate(request.coolingOffUntilUnix))}</p>` : ''}
      ${request.export?.ready ? '<div class="notice-card"><strong>Prepared, not downloadable here.</strong><p>The encrypted artifact exists, but the delivery bridge is not connected. This page intentionally provides no placeholder download link.</p></div>' : ''}
      <ol class="privacy-timeline">${timeline.map((event) => `<li><span class="privacy-timeline-dot" aria-hidden="true"></span><div><strong>${escapeHtml(pretty(event.eventType))}: ${escapeHtml(pretty(event.toStatus))}</strong><small>${escapeHtml(privacyDate(event.createdAtUnix))} · ${escapeHtml(pretty(event.actorType))}${event.reasonCode ? ` · ${escapeHtml(pretty(event.reasonCode))}` : ''}</small></div></li>`).join('') || '<li>No lifecycle events are available.</li>'}</ol>
      ${cancelForm}`;
  }

  async function loadPrivacy(root) {
    const reload = () => loadPrivacy(root);
    loading(root, 'Loading privacy and communication controls…');
    try {
      const [requests, consentCollection, communications] = await Promise.all([
        api('/api/privacy/requests?limit=50'),
        api('/api/privacy/consents'),
        api('/api/privacy/communications')
      ]);
      const notice = root._privacyNotice;
      root._privacyNotice = '';
      const consentByChannel = new Map((consentCollection.consents || []).filter((consent) => consent.purpose === 'marketing').map((consent) => [consent.channel, consent]));
      const communicationByKey = new Map(communications.map((item) => [`${item.channel}:${item.category}`, item]));
      root.innerHTML = `<section class="privacy-shell" aria-labelledby="privacy-heading">
        <div class="notice-card privacy-intro"><span class="eyebrow">Account privacy center</span><h2 id="privacy-heading">Your data, consent, and delivery choices</h2><p>Requests are tied to this signed-in account and universe. Essential security and transactional messages remain enabled so the account can operate safely.</p></div>
        <p class="privacy-global-feedback ${notice?.error ? 'is-error' : ''}" role="status" aria-live="polite">${notice ? escapeHtml(notice.message) : ''}</p>
        <section class="panel" aria-labelledby="privacy-actions-heading"><div class="panel-heading"><div><span class="eyebrow">Self-service requests</span><h2 id="privacy-actions-heading">Start a privacy request</h2></div></div>
          <div class="privacy-action-grid">
            <form class="privacy-action-card" data-privacy-request="export"><h3>Export my data</h3><p>Prepare a durable account export. Delivery is not connected yet; readiness is shown honestly in the timeline.</p><button type="submit">Request export</button><p class="form-feedback" role="status" aria-live="polite"></p></form>
            <form class="privacy-action-card" data-privacy-request="correction"><h3>Correct my data</h3><p>Open a reviewed correction request. Support will use the request timeline for follow-up.</p><button type="submit">Request correction</button><p class="form-feedback" role="status" aria-live="polite"></p></form>
            <form class="privacy-action-card danger-zone" data-privacy-request="restriction"><h3>Restrict my account</h3><p>Stops nonessential processing and communication and invalidates active access when applied.</p><label>Type <code>RESTRICT MY ACCOUNT</code><input name="confirmation" autocomplete="off" required></label><button type="submit" class="secondary">Request restriction</button><p class="form-feedback" role="status" aria-live="polite"></p></form>
            <form class="privacy-action-card danger-zone" data-privacy-request="erasure"><h3>Erase my account</h3><p>Starts the cooling-off, legal review, dual approval, and access-invalidation lifecycle. This is not immediate deletion.</p><label>Type <code>ERASE MY ACCOUNT</code><input name="confirmation" autocomplete="off" required></label><button type="submit" class="secondary">Request erasure</button><p class="form-feedback" role="status" aria-live="polite"></p></form>
          </div>
        </section>
        <section class="privacy-request-layout"><article class="panel"><div class="panel-heading"><div><span class="eyebrow">Durable history</span><h2>My privacy requests</h2></div><span class="count-badge">${formatNumber(requests.length)}</span></div><div class="privacy-request-list">${requests.length ? requests.map(privacyRequestMarkup).join('') : '<div class="empty-state compact">No privacy requests have been created for this account.</div>'}</div></article><aside class="panel privacy-detail" id="privacy-request-detail" aria-live="polite"><div class="empty-state">Select a request to inspect its immutable timeline and cancellation state.</div></aside></section>
        <section class="panel" aria-labelledby="privacy-consent-heading"><div class="panel-heading"><div><span class="eyebrow">Explicit consent · ${escapeHtml(consentCollection.currentPolicyVersion)}</span><h2 id="privacy-consent-heading">Marketing consent</h2></div></div><p class="contract-note">A communication preference alone never enables marketing. Current explicit consent for the active policy is also required. A channel-specific choice overrides the account-wide choice.</p><div class="privacy-consent-grid">${PRIVACY_CHANNELS.map((channel) => { const specific = consentByChannel.get(channel); const inherited = !specific ? consentByChannel.get('all') : null; const governing = specific || inherited; const granted = governing?.status === 'granted' && governing?.policyVersion === consentCollection.currentPolicyVersion && (!governing.expiresAtUnix || Number(governing.expiresAtUnix) > Date.now() / 1000); const provenance = specific ? `Channel-specific · ${pretty(specific.status)} · version ${formatNumber(specific.version)}` : inherited ? `Inherited account-wide · ${pretty(inherited.status)}; changing this creates a channel override` : 'No channel-specific or account-wide consent recorded'; return `<label class="privacy-toggle-row"><span><strong>${escapeHtml(pretty(channel))}</strong><small>${escapeHtml(provenance)}</small></span><input type="checkbox" role="switch" data-privacy-consent="${escapeHtml(channel)}" data-version="${escapeHtml(specific?.version || 0)}" ${granted ? 'checked' : ''} aria-label="Marketing consent by ${escapeHtml(pretty(channel))}"><span class="switch-track" aria-hidden="true"></span><span class="row-feedback" role="status" aria-live="polite"></span></label>`; }).join('')}</div></section>
        <section class="panel" aria-labelledby="privacy-communications-heading"><div class="panel-heading"><div><span class="eyebrow">4 channels × 5 categories</span><h2 id="privacy-communications-heading">Communication preferences</h2></div></div><div class="privacy-channel-grid">${PRIVACY_CHANNELS.map((channel) => `<section class="privacy-channel-card"><h3>${escapeHtml(pretty(channel))}</h3><div class="privacy-preference-list">${PRIVACY_CATEGORIES.map((category) => { const item = communicationByKey.get(`${channel}:${category}`); if (!item) return ''; const explanation = item.essential ? 'Required for account operation' : item.suppressedByRestriction ? 'Paused by account restriction' : category === 'marketing' && !item.marketingConsentCurrent ? 'Explicit marketing consent also required' : item.effectiveAllowed ? 'Delivery allowed' : 'Delivery paused'; return `<label class="privacy-toggle-row ${item.essential ? 'is-essential' : ''}"><span><strong>${escapeHtml(pretty(category))}</strong><small>${escapeHtml(explanation)}</small></span><input type="checkbox" role="switch" data-privacy-communication="${escapeHtml(channel)}:${escapeHtml(category)}" data-channel="${escapeHtml(channel)}" data-category="${escapeHtml(category)}" data-version="${escapeHtml(item.version)}" ${item.enabled ? 'checked' : ''} ${item.essential ? 'disabled aria-disabled="true"' : ''} aria-label="${escapeHtml(pretty(category))} by ${escapeHtml(pretty(channel))}"><span class="switch-track" aria-hidden="true"></span><span class="status-chip ${item.effectiveAllowed ? 'ally' : ''}">${item.essential ? 'Essential' : item.effectiveAllowed ? 'Effective' : 'Paused'}</span><span class="row-feedback" role="status" aria-live="polite"></span></label>`; }).join('')}</div></section>`).join('')}</div></section>
      </section>`;

      $$('[data-privacy-request]', root).forEach((form) => form.addEventListener('submit', (event) => {
        event.preventDefault();
        const requestType = form.dataset.privacyRequest;
        const confirmation = form.elements.confirmation?.value;
        const expectedPhrase = requestType === 'restriction' ? 'RESTRICT MY ACCOUNT' : requestType === 'erasure' ? 'ERASE MY ACCOUNT' : null;
        if (expectedPhrase && confirmation !== expectedPhrase) {
          feedback(form, `Type ${expectedPhrase} exactly to continue.`, true);
          form.elements.confirmation.focus();
          return;
        }
        const button = $('button[type="submit"]', form);
        withButtonLock(button, async () => {
          feedback(form, 'Recording your request…');
          try {
            await api('/api/privacy/requests', { method: 'POST', body: jsonBody({ requestType, idempotencyKey: privacyIdempotencyKey(root, requestType), ...(confirmation ? { confirmation } : {}) }) });
            delete root._privacyRequestKeys[requestType];
            root._privacyNotice = { message: `${pretty(requestType)} request recorded.`, error: false };
            await reload();
          } catch (error) {
            feedback(form, error.code === 'privacy_request_active' ? 'An active request of this type already exists. Review its timeline before creating another.' : error.status === 409 ? 'This request conflicts with current durable state. Review the request history and retry only if needed.' : error.message, true);
          }
        });
      }));

      const detailTarget = $('#privacy-request-detail', root);
      $$('.privacy-detail-button', root).forEach((button) => button.addEventListener('click', async () => {
        loading(detailTarget, 'Loading immutable request timeline…');
        try {
          const detail = await api(`/api/privacy/requests/${encodeURIComponent(button.dataset.requestDetail)}`);
          detailTarget.innerHTML = privacyTimelineMarkup(detail);
          finish(detailTarget);
          const cancelForm = $('.privacy-cancel-form', detailTarget);
          cancelForm?.addEventListener('submit', (event) => {
            event.preventDefault();
            const confirmation = cancelForm.elements.confirmation.value;
            if (confirmation !== 'CANCEL REQUEST') {
              feedback(cancelForm, 'Type CANCEL REQUEST exactly to continue.', true);
              return;
            }
            withButtonLock($('button[type="submit"]', cancelForm), async () => {
              try {
                await api(`/api/privacy/requests/${encodeURIComponent(cancelForm.dataset.requestId)}/cancel`, { method: 'POST', body: jsonBody({ expectedVersion: Number(cancelForm.dataset.requestVersion), confirmation }) });
                root._privacyNotice = { message: 'Privacy request cancelled and its active worker lease invalidated.', error: false };
                await reload();
              } catch (error) {
                feedback(cancelForm, privacyVersionConflict(error) ? 'This request changed before cancellation. Refresh the timeline and try again.' : error.message, true);
              }
            });
          });
        } catch (error) { failure(detailTarget, error, () => button.click()); }
      }));

      $$('[data-privacy-consent]', root).forEach((control) => control.addEventListener('change', () => {
        const row = control.closest('.privacy-toggle-row');
        const prior = !control.checked;
        withButtonLock(control, async () => {
          try {
            await api(`/api/privacy/consents/${encodeURIComponent(control.dataset.privacyConsent)}`, { method: 'PUT', body: jsonBody({ status: control.checked ? 'granted' : 'withdrawn', policyVersion: consentCollection.currentPolicyVersion, expectedVersion: Number(control.dataset.version), confirmed: control.checked }) });
            root._privacyNotice = { message: `Marketing consent for ${pretty(control.dataset.privacyConsent)} updated.`, error: false };
            await reload();
          } catch (error) {
            control.checked = prior;
            $('.row-feedback', row).textContent = privacyVersionConflict(error) ? 'Changed elsewhere; reload and retry.' : error.message;
            $('.row-feedback', row).classList.add('is-error');
          }
        });
      }));

      $$('[data-privacy-communication]', root).forEach((control) => control.addEventListener('change', () => {
        const row = control.closest('.privacy-toggle-row');
        const prior = !control.checked;
        withButtonLock(control, async () => {
          try {
            await api(`/api/privacy/communications/${encodeURIComponent(control.dataset.channel)}/${encodeURIComponent(control.dataset.category)}`, { method: 'PUT', body: jsonBody({ enabled: control.checked, expectedVersion: Number(control.dataset.version) }) });
            root._privacyNotice = { message: `${pretty(control.dataset.category)} preference for ${pretty(control.dataset.channel)} updated.`, error: false };
            await reload();
          } catch (error) {
            control.checked = prior;
            $('.row-feedback', row).textContent = privacyVersionConflict(error) ? 'Changed elsewhere; reload and retry.' : error.message;
            $('.row-feedback', row).classList.add('is-error');
          }
        });
      }));
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
    privacy: loadPrivacy,
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
