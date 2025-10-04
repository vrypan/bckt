(function () {
  'use strict';

  const root = document.querySelector('[data-search-root]');
  if (!root) {
    return;
  }

  const input = root.querySelector('[data-search-input]');
  const status = root.querySelector('[data-search-status]');
  const resultsContainer = root.querySelector('[data-search-results]');
  const filterElements = {
    language: root.querySelector('[data-search-filter="language"]'),
    type: root.querySelector('[data-search-filter="type"]'),
    tag: root.querySelector('[data-search-filter="tag"]'),
    year: root.querySelector('[data-search-filter="year"]'),
  };

  const indexUrl = root.getAttribute('data-search-index') || '/assets/search/search-index.json';
  let miniSearch = null;
  let documents = [];

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
      updateStatus('Type to search the site.');
      renderResults(miniSearch.collectAll(filterDocument));
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
    const results = miniSearch.search(query, {
      prefix: true,
      filter: filterDocument,
    });
    renderResults(results, query);
  }, 80);

  input.addEventListener('input', debouncedSearch);
  for (const element of Object.values(filterElements)) {
    if (element) {
      element.addEventListener('change', debouncedSearch);
    }
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

  function renderResults(results, query) {
    resultsContainer.innerHTML = '';
    if (!results || results.length === 0) {
      updateStatus(query ? 'No matches found.' : 'No posts to show.');
      return;
    }
    updateStatus(`${results.length} result${results.length === 1 ? '' : 's'}${query ? ` for “${query}”` : ''}.`);

    const fragment = document.createDocumentFragment();
    for (const result of results) {
      fragment.appendChild(renderResultCard(result));
    }
    resultsContainer.appendChild(fragment);
  }

  function renderResultCard(result) {
    const article = document.createElement('article');
    article.className = 'search-card';

    const header = document.createElement('header');
    header.className = 'search-card__header';

    const meta = document.createElement('p');
    meta.className = 'search-card__meta';
    meta.innerHTML = [formatDate(result.date_display || result.date_iso), result.type, result.language]
      .filter(Boolean)
      .map((value) => `<span>${escapeHtml(value)}</span>`)
      .join(' <span class="meta-divider">•</span> ');
    header.appendChild(meta);

    const heading = document.createElement('h2');
    heading.className = 'search-card__title';
    const link = document.createElement('a');
    link.href = result.url || result.id;
    link.textContent = result.title || result.id;
    heading.appendChild(link);
    header.appendChild(heading);
    article.appendChild(header);

    const body = document.createElement('div');
    body.className = 'search-card__body';
    body.textContent = result.excerpt || result.content.slice(0, 160);
    article.appendChild(body);

    if (Array.isArray(result.tags) && result.tags.length > 0) {
      const footer = document.createElement('footer');
      footer.className = 'search-card__tags';
      for (const tag of result.tags) {
        const badge = document.createElement('span');
        badge.className = 'search-tag';
        badge.textContent = tag;
        footer.appendChild(badge);
      }
      article.appendChild(footer);
    }

    return article;
  }

  function formatDate(value) {
    return value || '';
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
})();
