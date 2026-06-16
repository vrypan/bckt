(function (global) {
  'use strict';

  function asArray(value) {
    if (Array.isArray(value)) {
      return value;
    }
    if (value === null || value === undefined) {
      return [];
    }
    return [value];
  }

  class MiniSearch {
    constructor(options = {}) {
      this.fields = Array.isArray(options.fields) ? options.fields : [];
      this.invertedIndex = new Map();
      this.documents = new Map();
      this.tokenPattern = /[\p{L}\p{N}]+/gu;
    }

    addAll(documents) {
      if (!Array.isArray(documents)) {
        return;
      }
      for (const document of documents) {
        this.add(document);
      }
    }

    add(document) {
      if (!document || typeof document !== 'object') {
        return;
      }
      const id = document.id;
      if (id === undefined || id === null) {
        throw new Error('MiniSearch: document is missing id field');
      }

      this.documents.set(id, document);

      for (const field of this.fields) {
        const value = document[field];
        if (value === undefined || value === null) {
          continue;
        }
        const tokens = this.tokenize(asArray(value).join(' '));
        for (const token of tokens) {
          let docMap = this.invertedIndex.get(token);
          if (!docMap) {
            docMap = new Map();
            this.invertedIndex.set(token, docMap);
          }
          docMap.set(id, (docMap.get(id) || 0) + 1);
        }
      }
    }

    search(query, options = {}) {
      const filterFn = typeof options.filter === 'function' ? options.filter : null;
      const enablePrefix = Boolean(options.prefix);

      const terms = this.tokenize(query);
      if (terms.length === 0) {
        return this.collectAll(filterFn);
      }

      const matchGroups = terms.map((term) => this.lookupTerm(term, enablePrefix));
      if (matchGroups.some((group) => group.length === 0)) {
        return [];
      }

      const candidateIds = this.intersection(matchGroups);
      if (candidateIds.size === 0) {
        return [];
      }

      const results = [];
      for (const id of candidateIds) {
        const document = this.documents.get(id);
        if (!document) {
          continue;
        }
        if (filterFn && !filterFn(document)) {
          continue;
        }
        const score = this.scoreDocument(id, matchGroups);
        results.push(Object.assign({ score }, document));
      }

      results.sort((a, b) => {
        if (b.score !== a.score) {
          return b.score - a.score;
        }
        const left = typeof a.timestamp === 'number' ? a.timestamp : 0;
        const right = typeof b.timestamp === 'number' ? b.timestamp : 0;
        return right - left;
      });

      return results;
    }

    collectAll(filterFn) {
      const results = [];
      for (const document of this.documents.values()) {
        if (filterFn && !filterFn(document)) {
          continue;
        }
        results.push(Object.assign({ score: 0 }, document));
      }
      results.sort((a, b) => {
        const left = typeof a.timestamp === 'number' ? a.timestamp : 0;
        const right = typeof b.timestamp === 'number' ? b.timestamp : 0;
        return right - left;
      });
      return results;
    }

    lookupTerm(term, prefix) {
      if (!prefix) {
        const entry = this.invertedIndex.get(term);
        return entry ? [{ token: term, docs: entry }] : [];
      }
      const matches = [];
      for (const [token, docs] of this.invertedIndex.entries()) {
        if (token.startsWith(term)) {
          matches.push({ token, docs });
        }
      }
      return matches;
    }

    intersection(matchGroups) {
      let accumulator = null;
      for (const group of matchGroups) {
        const ids = new Set();
        for (const match of group) {
          for (const docId of match.docs.keys()) {
            ids.add(docId);
          }
        }
        if (accumulator === null) {
          accumulator = ids;
        } else {
          const next = new Set();
          for (const id of accumulator) {
            if (ids.has(id)) {
              next.add(id);
            }
          }
          accumulator = next;
        }
        if (accumulator.size === 0) {
          break;
        }
      }
      return accumulator || new Set();
    }

    scoreDocument(id, matchGroups) {
      let score = 0;
      for (const group of matchGroups) {
        for (const match of group) {
          const count = match.docs.get(id);
          if (count) {
            score += count;
          }
        }
      }
      return score;
    }

    tokenize(raw) {
      if (!raw) {
        return [];
      }
      const text = String(raw).toLowerCase();
      let tokens = text.match(this.tokenPattern);
      if (!tokens) {
        tokens = text.match(/[a-z0-9]+/g) || [];
      }
      return tokens.map((token) => token.toLowerCase());
    }
  }

  if (typeof module !== 'undefined' && module.exports) {
    module.exports = MiniSearch;
  } else {
    global.MiniSearch = MiniSearch;
  }
})(typeof window !== 'undefined' ? window : globalThis);
