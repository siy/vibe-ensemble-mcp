import { createSignal, onMount } from 'solid-js';

function ThemeToggle() {
  const [isDark, setIsDark] = createSignal(true);

  onMount(() => {
    // Detect system preference
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const savedTheme = localStorage.getItem('theme');
    const initialDark = savedTheme === 'dark' || (!savedTheme && prefersDark);

    setIsDark(initialDark);
    document.documentElement.setAttribute('data-theme', initialDark ? 'dark' : 'light');
  });

  function toggleTheme() {
    const newIsDark = !isDark();
    setIsDark(newIsDark);
    const theme = newIsDark ? 'dark' : 'light';
    document.documentElement.setAttribute('data-theme', theme);
    localStorage.setItem('theme', theme);
  }

  return (
    <button
      onClick={toggleTheme}
      aria-label="Toggle theme"
      title={isDark() ? 'Switch to light mode' : 'Switch to dark mode'}
      style={{ width: 'auto', padding: '0.5rem 1rem' }}
    >
      {isDark() ? '☀️ Light' : '🌙 Dark'}
    </button>
  );
}

export default ThemeToggle;
