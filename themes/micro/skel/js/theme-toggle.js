(() => {
  const toggle = document.querySelector('[data-theme-toggle]');
  const label = document.querySelector('[data-theme-toggle-label]');

  if (!toggle || !label) {
    return;
  }

  function setTheme(theme) {
    const isDark = theme === 'dark';
    document.documentElement.dataset.theme = theme;
    toggle.setAttribute('aria-pressed', String(isDark));
    label.textContent = isDark ? 'Light' : 'Dark';
  }

  const initialTheme = document.documentElement.dataset.theme || 'light';
  setTheme(initialTheme);

  toggle.addEventListener('click', () => {
    const nextTheme = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
    try {
      localStorage.setItem('bckt-color-theme', nextTheme);
    } catch (error) {
      // Ignore storage failures; the in-page toggle should still work.
    }
    setTheme(nextTheme);
  });
})();
