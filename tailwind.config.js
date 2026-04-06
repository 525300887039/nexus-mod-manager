/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['./src/**/*.{js,jsx,html}'],
  theme: {
    extend: {
      colors: {
        primary: '#1a1a1a',
        accent: {
          green: '#e8f5e9',
          yellow: '#fff8e1',
          pink: '#fce4ec',
          blue: '#e3f2fd',
          purple: '#f3e5f5',
        },
      },
      fontFamily: {
        sans: ['Inter', 'system-ui', '-apple-system', 'sans-serif'],
      },
    },
  },
  plugins: [],
};
