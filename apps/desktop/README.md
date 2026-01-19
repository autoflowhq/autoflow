# Electron + React + TypeScript App

This project is a desktop application using Electron, React, and TypeScript, bootstrapped with Vite.

## Scripts

- `npm run dev` — Start Vite dev server and Electron for development
- `npm run build` — Build the React app
- `npm run electron` — Run Electron with the production build
- `npm run lint` — Lint the codebase

## Structure

- `electron-main.js` — Main process for Electron
- `preload.js` — Preload script for Electron
- `src/` — React app source code

## Getting Started

1. Install dependencies:
  ```sh
  npm install
  ```
2. Start development:
  ```sh
  npm run dev
  ```

## Build for Production

1. Build the React app:
  ```sh
  npm run build
  ```
2. Start Electron:
  ```sh
  npm run electron
  ```
    },
  },
])
```

You can also install [eslint-plugin-react-x](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-x) and [eslint-plugin-react-dom](https://github.com/Rel1cx/eslint-react/tree/main/packages/plugins/eslint-plugin-react-dom) for React-specific lint rules:

```js
// eslint.config.js
import reactX from 'eslint-plugin-react-x'
import reactDom from 'eslint-plugin-react-dom'

export default defineConfig([
  globalIgnores(['dist']),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      // Other configs...
      // Enable lint rules for React
      reactX.configs['recommended-typescript'],
      // Enable lint rules for React DOM
      reactDom.configs.recommended,
    ],
    languageOptions: {
      parserOptions: {
        project: ['./tsconfig.node.json', './tsconfig.app.json'],
        tsconfigRootDir: import.meta.dirname,
      },
      // other options...
    },
  },
])
```
