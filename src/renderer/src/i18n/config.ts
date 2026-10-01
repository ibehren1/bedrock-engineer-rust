import i18n from 'i18next'
import { initReactI18next } from 'react-i18next'
import en from './locales/en'
import ja from './locales/ja'

const defaultLaunguage = window.store.get('language') ?? navigator.language

const resources = {
  en: {
    translation: en
  },
  ja: {
    translation: ja
  }
}

i18n.use(initReactI18next).init({
  resources,
  lng: defaultLaunguage,
  interpolation: {
    escapeValue: false,
    // App display name comes from package.json productName (injected at build time),
    // so every `{{appName}}` placeholder resolves without per-call arguments.
    defaultVariables: {
      appName: __APP_NAME__
    }
  }
})

export default i18n
