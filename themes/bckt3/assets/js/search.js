(function () {
  'use strict';

  const root = document.querySelector('[data-search-root]');
  if (!root) {
    return;
  }

  const input = root.querySelector('[data-search-input]');
  const status = root.querySelector('[data-search-status]');
  const resultsContainer = root.querySelector('[data-search-results]');
  const filtersContainer = root.querySelector('[data-search-filters]');
  const toggleButton = root.querySelector('[data-search-toggle]');
  const sortSelect = root.querySelector('[data-search-sort]');
  const resultTemplate = root.querySelector('[data-search-result-template]');
  const filterElements = {
    language: root.querySelector('[data-search-filter="language"]'),
    type: root.querySelector('[data-search-filter="type"]'),
    tag: root.querySelector('[data-search-filter="tag"]'),
    year: root.querySelector('[data-search-filter="year"]'),
  };

  const baseUrl = root.getAttribute('data-base-url') || '';
  const indexUrl = root.getAttribute('data-search-index') || baseUrl + '/assets/search/search-index.json';
  let miniSearch = null;
  let documents = [];
  let lastBaseResults = [];
  let lastTokens = [];
  let lastQuery = '';

  updateStatus('Loading search index…');

  fetch(indexUrl, { credentials: 'same-origin' })
    .then((response) => {
      if (!response.ok) {
        throw new Error('Failed to load search index');
      }
      return response.json();
    })
    .then((payload) => {
      documents = (payload.documents || []).map((doc) => (
        Object.assign({}, doc, {
          tags_text: Array.isArray(doc.tags) ? doc.tags.join(' ') : '',
          excerpt: doc.excerpt || '',
          type: doc.type || '',
        })
      ));
      miniSearch = new MiniSearch({
        fields: ['title', 'content', 'excerpt', 'tags_text'],
        prefix: true,
      });
      miniSearch.addAll(documents);
      buildFilters(payload);
      renderResults([], [], '');
      updateStatus('Type a keyword to start searching.');
    })
    .catch((error) => {
      console.error(error);
      updateStatus('Search is unavailable right now.');
    });

  const debouncedSearch = debounce(() => {
    if (!miniSearch) {
      return;
    }
    const query = (input.value || '').trim();
    if (!query) {
      lastBaseResults = [];
      lastTokens = [];
      lastQuery = '';
      renderResults([], [], '');
      updateStatus('Type a keyword to start searching.');
      return;
    }

    const tokens = dedupe(miniSearch.tokenize(query).filter(Boolean));
    const baseResults = miniSearch.search(query, {
      prefix: true,
      filter: filterDocument,
    });
    lastBaseResults = baseResults;
    lastTokens = tokens;
    lastQuery = query;
    const sorted = applySort(baseResults.slice());
    renderResults(sorted, tokens, query);
  }, 120);

  input.addEventListener('input', debouncedSearch);
  for (const element of Object.values(filterElements)) {
    if (element) {
      element.addEventListener('change', debouncedSearch);
    }
  }

  if (sortSelect) {
    sortSelect.addEventListener('change', () => {
      if (!miniSearch || !lastQuery) {
        return;
      }
      const sorted = applySort(lastBaseResults.slice());
      renderResults(sorted, lastTokens, lastQuery);
    });
  }

  if (toggleButton && filtersContainer) {
    toggleButton.addEventListener('click', () => {
      const expanded = toggleButton.getAttribute('aria-expanded') === 'true';
      const next = !expanded;
      toggleButton.setAttribute('aria-expanded', String(next));
      if (next) {
        filtersContainer.removeAttribute('hidden');
        toggleButton.textContent = 'Hide options';
      } else {
        filtersContainer.setAttribute('hidden', '');
        toggleButton.textContent = 'More options';
      }
    });
  }

  function buildFilters(payload) {
    const languageItems = (payload.languages || []).map((entry) => ({
      value: entry && entry.id ? entry.id : entry,
      label: entry && entry.name ? entry.name : entry && entry.id ? entry.id : entry,
    }));
    const facets = payload.facets || {};
    populateSelect(filterElements.language, languageItems, 'Language');
    populateSelect(filterElements.type, (facets.types || []).map((value) => ({ value, label: value })), 'Type');
    populateSelect(filterElements.tag, (facets.tags || []).map((value) => ({ value, label: value })), 'Tag');
    populateSelect(
      filterElements.year,
      (facets.years || []).map((value) => ({ value: value, label: String(value) })),
      'Year',
      true
    );
  }

  function populateSelect(select, values, label, numeric) {
    if (!select) {
      return;
    }
    select.innerHTML = '';
    const option = document.createElement('option');
    option.value = '';
    option.textContent = `All ${label.toLowerCase()}s`;
    select.appendChild(option);
    const items = values
      .map((entry) => {
        if (entry && typeof entry === 'object') {
          return {
            value: entry.value,
            label: entry.label !== undefined ? entry.label : entry.value,
          };
        }
        return { value: entry, label: entry };
      })
      .filter((entry) => entry.value !== undefined && entry.value !== null);

    items.sort((a, b) => {
      if (numeric) {
        return Number(b.value) - Number(a.value);
      }
      return String(a.label).localeCompare(String(b.label));
    });

    for (const item of items) {
      const opt = document.createElement('option');
      opt.value = item.value;
      opt.textContent = item.label;
      select.appendChild(opt);
    }
  }

  function filterDocument(doc) {
    if (!doc) {
      return false;
    }
    const language = filterElements.language && filterElements.language.value;
    const type = filterElements.type && filterElements.type.value;
    const tag = filterElements.tag && filterElements.tag.value;
    const year = filterElements.year && filterElements.year.value;

    if (language && doc.language !== language) {
      return false;
    }
    if (type && (doc.type || '') !== type) {
      return false;
    }
    if (tag && !(Array.isArray(doc.tags) && doc.tags.includes(tag))) {
      return false;
    }
    if (year) {
      const docYear = doc.date_iso ? doc.date_iso.substring(0, 4) : '';
      if (docYear !== year) {
        return false;
      }
    }
    return true;
  }

  function renderResults(results, tokens, query) {
    resultsContainer.innerHTML = '';
    if (!results || results.length === 0) {
      if (query && query.length) {
        updateStatus(`No matches found for “${query}”.`);
      }
      return;
    }
    updateStatus(`${results.length} result${results.length === 1 ? '' : 's'} for “${query}”.`);

    const fragment = document.createDocumentFragment();
    for (const result of results) {
      fragment.appendChild(renderResultCard(result, tokens));
    }
    resultsContainer.appendChild(fragment);
  }

  function applySort(results) {
    if (!sortSelect) {
      return results;
    }
    const mode = sortSelect.value;
    if (mode === 'newest') {
      results.sort((a, b) => (b.timestamp || 0) - (a.timestamp || 0));
    }
    return results;
  }

  function renderResultCard(result, tokens) {
    if (resultTemplate) {
      return renderFromTemplate(result, tokens);
    }
    const isFarcaster = (result.type || '').toLowerCase() === 'farcaster';
    const article = document.createElement('article');
    article.className = 'post-card';
    if (isFarcaster) {
      article.classList.add('post-card--farcaster');
      renderFarcasterSummary(article, result, tokens);
    } else {
      renderDefaultSummary(article, result, tokens);
    }
    return article;
  }

  function renderFromTemplate(result, tokens) {
    const clone = resultTemplate.content.cloneNode(true);
    const card = clone.querySelector('[data-search-card]') || clone.firstElementChild;
    if (card) {
      card.classList.remove('post-card--farcaster');
      if ((result.type || '').toLowerCase() === 'farcaster') {
        card.classList.add('post-card--farcaster');
      }
    }

    const url = result.url || result.id || '#';

    const titleNode = clone.querySelector('[data-search-title]');
    if (titleNode) {
      if (result.title) {
        titleNode.innerHTML = highlightText(result.title, tokens);
        titleNode.removeAttribute('hidden');
      } else {
        titleNode.innerHTML = '';
        titleNode.setAttribute('hidden', '');
      }
    }

    const summaryNode = clone.querySelector('[data-search-summary]');
    const summaryDivider = clone.querySelector('[data-search-summary-divider]');
    const summarySource = (result.excerpt || result.content || '').trim();
    if (summaryNode) {
      if (summarySource.length > 0) {
        summaryNode.innerHTML = highlightText(summarySource, tokens);
        summaryNode.removeAttribute('hidden');
        if (summaryDivider) {
          summaryDivider.removeAttribute('hidden');
        }
      } else {
        summaryNode.innerHTML = '';
        summaryNode.setAttribute('hidden', '');
        if (summaryDivider) {
          summaryDivider.setAttribute('hidden', '');
        }
      }
    } else if (summaryDivider) {
      summaryDivider.remove();
    }

    const dateNode = clone.querySelector('[data-search-date]');
    if (dateNode) {
      const dateIso = result.date_iso || '';
      const display = result.date_display || formatDate(dateIso);
      if ('dateTime' in dateNode) {
        dateNode.dateTime = dateIso;
      } else if (dateIso) {
        dateNode.setAttribute('data-date-iso', dateIso);
      }
      dateNode.textContent = display;
    }

    const tagTarget = clone.querySelector('[data-search-tags]');
    if (tagTarget) {
      populateTags(tagTarget, Array.isArray(result.tags) ? result.tags : []);
    }

    clone.querySelectorAll('[data-search-link]').forEach((element) => {
      if ('href' in element) {
        element.href = url;
      } else {
        element.setAttribute('data-href', url);
      }
    });

    const payload = result.payload || {};
    clone.querySelectorAll('[data-search-payload]').forEach((element) => {
      const key = element.getAttribute('data-search-payload');
      if (!key) {
        return;
      }
      const value = payload[key];
      if (value === undefined || value === null) {
        element.setAttribute('hidden', '');
        return;
      }
      const attrTarget = element.getAttribute('data-search-payload-attr');
      if (attrTarget) {
        element.setAttribute(attrTarget, String(value));
        element.removeAttribute('hidden');
        return;
      }
      if (element.tagName === 'IMG' && typeof value === 'string') {
        element.setAttribute('src', value);
        element.removeAttribute('hidden');
        return;
      }
      if (typeof value === 'object') {
        element.textContent = JSON.stringify(value);
      } else if ('textContent' in element) {
        element.textContent = String(value);
      }
      element.removeAttribute('hidden');
    });

    return clone;
  }

  function renderDefaultSummary(container, result, tokens) {
    const header = document.createElement('header');
    const meta = document.createElement('div');
    meta.className = 'post-meta post-meta--compact';

    const time = document.createElement('time');
    time.className = 'post-meta__time';
    time.dateTime = result.date_iso || '';
    time.textContent = formatDate(result.date_display || result.date_iso);
    meta.appendChild(time);

    if (result.title) {
      meta.appendChild(createDivider('·'));
      const titleLink = document.createElement('a');
      titleLink.className = 'post-card__title';
      titleLink.href = result.url || result.id;
      titleLink.innerHTML = highlightText(result.title, tokens);
      titleLink.rel = 'bookmark';
      meta.appendChild(titleLink);
    }

    if (Array.isArray(result.tags) && result.tags.length > 0) {
      meta.appendChild(createDivider('•'));
      const list = document.createElement('ul');
      list.className = 'post-tags post-tags--compact';
      for (const tag of result.tags) {
        const item = document.createElement('li');
        const link = document.createElement('a');
        link.href = `${baseUrl}/tags/${tagSlug(tag)}/`;
        link.textContent = `#${tag}`;
        item.appendChild(link);
        list.appendChild(item);
      }
      meta.appendChild(list);
    }

    const summarySource = result.excerpt || result.content || '';
    if (summarySource.trim().length > 0) {
      meta.appendChild(createDivider('·'));
      const summary = document.createElement('span');
      summary.className = 'post-card__abstract';
      summary.innerHTML = highlightText(summarySource, tokens);
      summary.appendChild(document.createTextNode(' '));
      summary.appendChild(document.createTextNode('['));
      const continueLink = document.createElement('a');
      continueLink.className = 'post-card__continue';
      continueLink.href = result.url || result.id;
      continueLink.setAttribute('aria-label', 'Read full post');
      continueLink.textContent = 'Read→';
      summary.appendChild(continueLink);
      summary.appendChild(document.createTextNode(']'));
      meta.appendChild(summary);
    }

    header.appendChild(meta);
    container.appendChild(header);
  }

  function populateTags(target, tags) {
    target.innerHTML = '';
    if (!Array.isArray(tags) || tags.length === 0) {
      target.setAttribute('hidden', '');
      return;
    }
    target.removeAttribute('hidden');
    const nodeName = target.nodeName.toLowerCase();
    if (nodeName === 'ul' || nodeName === 'ol') {
      for (const tag of tags) {
        const item = document.createElement('li');
        const link = document.createElement('a');
        link.href = `${baseUrl}/tags/${tagSlug(tag)}/`;
        link.textContent = `#${tag}`;
        item.appendChild(link);
        target.appendChild(item);
      }
    } else {
      target.textContent = tags.map((tag) => `#${tag}`).join(', ');
    }
  }

  function renderFarcasterSummary(container, result, tokens) {
    const body = document.createElement('div');
    body.className = 'post-card__body';

    const meta = document.createElement('div');
    meta.className = 'post-meta post-meta--inline';
    const time = document.createElement('time');
    time.className = 'post-meta__time';
    time.dateTime = result.date_iso || '';
    time.textContent = formatDate(result.date_display || result.date_iso);
    meta.appendChild(time);
    meta.appendChild(createDivider('·'));
    const origin = document.createElement('span');
    origin.textContent = 'on farcaster';
    meta.appendChild(origin);
    body.appendChild(meta);

    const content = document.createElement('div');
    content.className = 'post-card__content reading-flow';
    const highlighted = highlightText(result.excerpt || result.content || '', tokens);
    if (highlighted.length > 0) {
      const segments = highlighted.split(/\n{2,}/).map((segment) => segment.replace(/\n/g, '<br>'));
      content.innerHTML = segments
        .filter((segment) => segment.trim().length > 0)
        .map((segment) => `<p>${segment}</p>`)
        .join('');
    }
    body.appendChild(content);
    container.appendChild(body);
  }

  function formatDate(value) {
    return value || '';
  }

  function createDivider(symbol) {
    const divider = document.createElement('span');
    divider.className = 'meta-divider';
    divider.textContent = symbol;
    return divider;
  }

  function escapeHtml(value) {
    return String(value).replace(/[&<>"']/g, (ch) => {
      switch (ch) {
        case '&':
          return '&amp;';
        case '<':
          return '&lt;';
        case '>':
          return '&gt;';
        case '"':
          return '&quot;';
        case '\'':
          return '&#39;';
        default:
          return ch;
      }
    });
  }

  function debounce(fn, wait) {
    let timeoutId = null;
    return function debounced() {
      const args = arguments;
      if (timeoutId !== null) {
        clearTimeout(timeoutId);
      }
      timeoutId = setTimeout(() => {
        timeoutId = null;
        fn.apply(null, args);
      }, wait);
    };
  }

  function updateStatus(message) {
    if (!status) {
      return;
    }
    status.textContent = message;
  }

  function highlightText(text, tokens) {
    const source = text || '';
    if (!source.trim() || !tokens || tokens.length === 0) {
      return escapeHtml(source);
    }
    const sorted = tokens
      .slice()
      .filter(Boolean)
      .sort((a, b) => b.length - a.length);
    let highlighted = escapeHtml(source);
    for (const token of sorted) {
      const pattern = new RegExp(`(${escapeRegex(token)})`, 'gi');
      highlighted = highlighted.replace(pattern, '<mark>$1</mark>');
    }
    return highlighted;
  }

  function escapeRegex(value) {
    return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  }

  function tagSlug(tag) {
    return String(tag || '')
      .trim()
      .toLowerCase()
      .replace(/\s+/g, '-');
  }

  function dedupe(list) {
    const seen = new Set();
    const result = [];
    for (const item of list) {
      const lower = String(item).toLowerCase();
      if (!seen.has(lower)) {
        seen.add(lower);
        result.push(lower);
      }
    }
    return result;
  }
})();
