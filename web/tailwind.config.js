/** @type {import('tailwindcss').Config} */
module.exports = {
  content: [
    "./index.html",
    "./src/**/*.rs",
    "./.tailwind",
  ],
  darkMode: "selector",
  theme: {
    extend: {
      colors: {
        "oa-blue-lighter": "#5bb8dc",
        "oa-blue": "#32a6d3",
        "oa-blue-darker": "#00729c",
        "oa-red": "#e52323",
        "oa-red-darker": "#be1717",
        "oa-gray": "#e6e6e6",
        "oa-gray-mid": "#d6d6d6",
        "oa-gray-darker": "#c3c3c3",
        "oa-docs-blue": "#30638e",
        "codeblock-light": "#fafdff",
        "codeblock-dark": "#04121B",
        success: "var(--success, #16a34a)",
        "success-soft": "var(--success-soft, #dcfce7)",
        warning: "var(--warning, #ca8a04)",
        "warning-soft": "var(--warning-soft, #fef9c3)",
        danger: "var(--danger, #dc2626)",
        "danger-soft": "var(--danger-soft, #fee2e2)",
      },
      aria: {
        "current-page": 'current="page"',
      },
    },
  },
  plugins: [],
};
