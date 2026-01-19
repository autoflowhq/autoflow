// ESLint configuration for Electron + React + TypeScript
module.exports = {
  root: true,
  env: {
    browser: true,
    node: true,
    es2021: true,
  },
  extends: [
    'eslint:recommended',
    'plugin:react/recommended',
    'plugin:@typescript-eslint/recommended',
  ],
  parser: '@typescript-eslint/parser',
  plugins: ['react', '@typescript-eslint'],
  rules: {
    // Customize rules as needed
  },
  settings: {
    react: {
      version: 'detect',
    },
  },
};
